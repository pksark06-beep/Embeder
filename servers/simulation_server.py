# SPDX-License-Identifier: MPL-2.0
"""Embeder Simulation MCP server (ADR-0001).

Exposes `run_simulation` over MCP: boots an ELF on a Renode platform headlessly,
captures a UART to a file, and reports raw results. The Rust Core assigns the
confidence tier and verifiability boundary (it is the tiering authority).
"""
import os
import sys
import tempfile
import subprocess
import re

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from mcp_lib import serve  # noqa: E402

REPO_ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
WORKSPACE_ROOT = os.path.realpath(os.environ.get("EMBEDER_WORKSPACE_ROOT", REPO_ROOT))
DEFAULT_RENODE = os.environ.get("EMBEDER_RENODE", r"C:\Program Files\Renode\bin\Renode.exe")


def _within(path, root):
    try:
        return os.path.commonpath([os.path.realpath(path), root]) == root
    except ValueError:
        return False


def _safe_renode(candidate):
    requested = os.path.realpath(candidate)
    configured = os.path.realpath(DEFAULT_RENODE)
    if os.path.basename(candidate).lower() == "renode" or requested == configured:
        return candidate
    raise ValueError("Renode executable is not allowed by the simulation sandbox")


def _safe_artifact(path):
    real = os.path.realpath(path)
    temp_root = os.path.realpath(tempfile.gettempdir())
    if not (_within(real, WORKSPACE_ROOT) or _within(real, temp_root)):
        raise ValueError("simulation artifact is outside the sandbox")
    if not os.path.isfile(real):
        raise ValueError("simulation artifact does not exist")
    return real


def _fwd(p):
    return p.replace("\\", "/")


def run_simulation(renode_bin="renode", platform="@platforms/boards/stm32f4_discovery.repl",
                   uart="sysbus.usart2", elf_path="", run_for_secs="0.5",
                   expect="", timeout_secs=180):
    renode_bin = _safe_renode(renode_bin)
    elf_path = _safe_artifact(elf_path)
    if not platform.startswith("@platforms/"):
        raise ValueError("only built-in Renode platforms are allowed")
    if not re.fullmatch(r"[A-Za-z0-9_.]+", uart):
        raise ValueError("invalid UART path")
    run_seconds = float(run_for_secs)
    if not 0 < run_seconds <= 10:
        raise ValueError("simulation duration is outside the sandbox limit")
    timeout_secs = max(1, min(int(timeout_secs), 180))

    build_dir = tempfile.mkdtemp(prefix="embeder-mcp-sim-")
    uart_out = os.path.join(build_dir, "uart.txt")
    resc = os.path.join(build_dir, "run.resc")

    script = (
        'mach create "embeder"\n'
        f"machine LoadPlatformDescription {platform}\n"
        f"sysbus LoadELF @{_fwd(elf_path)}\n"
        f"{uart} CreateFileBackend @{_fwd(uart_out)}\n"
        f'emulation RunFor "{run_for_secs}"\n'
        f"{uart} CloseFileBackend @{_fwd(uart_out)}\n"
        "quit\n"
    )
    with open(resc, "w", encoding="utf-8") as f:
        f.write(script)

    timed_out = False
    code = None
    out = ""
    try:
        proc = subprocess.run(
            [renode_bin, "--console", "--disable-gui", "--plain", resc],
            capture_output=True, text=True, timeout=timeout_secs,
        )
        code = proc.returncode
        out = (proc.stdout or "") + (proc.stderr or "")
    except FileNotFoundError:
        return {"ok": False, "available": False, "error": f"{renode_bin} not found",
                "uart": "", "uart_bytes": 0, "timed_out": False}
    except subprocess.TimeoutExpired as e:
        timed_out = True
        out = str(e)

    uart_text = ""
    try:
        with open(uart_out, encoding="utf-8", errors="replace") as f:
            uart_text = f.read()
    except OSError:
        pass

    ok = (not timed_out) and (expect in uart_text if expect else True)
    return {
        "ok": ok,
        "available": True,
        "timed_out": timed_out,
        "returncode": code,
        "uart": uart_text,
        "uart_bytes": len(uart_text),
        "engine": "renode",
        "log": out[:900],
    }


TOOLS = {
    "run_simulation": {
        "fn": run_simulation,
        "desc": "Boot an ELF on a Renode platform headlessly and capture a UART.",
        "schema": {
            "type": "object",
            "properties": {
                "renode_bin": {"type": "string"},
                "platform": {"type": "string"},
                "uart": {"type": "string"},
                "elf_path": {"type": "string"},
                "run_for_secs": {"type": "string"},
                "expect": {"type": "string"},
                "timeout_secs": {"type": "integer"},
            },
            "required": ["elf_path"],
        },
    },
}


if __name__ == "__main__":
    serve("embeder-simulation", TOOLS)
