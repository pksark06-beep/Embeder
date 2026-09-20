# SPDX-License-Identifier: MPL-2.0
"""Embeder Firmware MCP server (ADR-0001).

Exposes `compile_firmware` over MCP: a thin, sandboxable executor around a native
toolchain (arm-none-eabi-gcc). It runs the build and returns raw results; the Rust
Core parses stderr into structured diagnostics with its own (tested) parser, so the
interpretation logic stays in one place.
"""
import os
import sys
import tempfile
import subprocess

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from mcp_lib import serve  # noqa: E402

REPO_ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
WORKSPACE_ROOT = os.path.realpath(os.environ.get("EMBEDER_WORKSPACE_ROOT", REPO_ROOT))
ALLOWED_COMPILERS = {"arm-none-eabi-gcc", "arm-none-eabi-gcc.exe"}
BLOCKED_FLAG_PREFIXES = ("@", "-fplugin", "-specs", "-wrapper", "-B", "-o")


def _within(path, root):
    try:
        return os.path.commonpath([os.path.realpath(path), root]) == root
    except ValueError:
        return False


def _safe_generated_path(build_dir, name):
    if not isinstance(name, str) or not name or os.path.isabs(name):
        raise ValueError("generated source path must be relative")
    path = os.path.realpath(os.path.join(build_dir, name))
    if not _within(path, os.path.realpath(build_dir)):
        raise ValueError("generated source escaped the build sandbox")
    return path


def _safe_workspace_input(path, label):
    real = os.path.realpath(path)
    if not _within(real, WORKSPACE_ROOT):
        raise ValueError(f"{label} must stay inside the Embeder workspace")
    return real


def _safe_flags(values):
    clean = []
    for value in values:
        if not isinstance(value, str) or len(value) > 256 or "\x00" in value or "\n" in value:
            raise ValueError("invalid compiler flag")
        if value.startswith(BLOCKED_FLAG_PREFIXES):
            raise ValueError(f"compiler flag is blocked by the sandbox: {value}")
        clean.append(value)
    return clean


def compile_firmware(cc="arm-none-eabi-gcc", cpu_flags=None, extra_flags=None,
                     support_sources=None, linker_script=None,
                     target="unknown", entry="main.c", files=None):
    if os.path.basename(cc).lower() not in ALLOWED_COMPILERS:
        raise ValueError("compiler is not allowed by the firmware sandbox")
    cpu_flags = _safe_flags(cpu_flags or [])
    extra_flags = _safe_flags(extra_flags or [])
    support_sources = [_safe_workspace_input(path, "support source") for path in (support_sources or [])]
    if linker_script:
        linker_script = _safe_workspace_input(linker_script, "linker script")
    files = files or {}

    build_dir = tempfile.mkdtemp(prefix="embeder-mcp-build-")
    sources = []
    for name, content in files.items():
        if not isinstance(content, str) or len(content.encode("utf-8")) > 512 * 1024:
            raise ValueError("generated source exceeds the sandbox file limit")
        path = _safe_generated_path(build_dir, name)
        parent = os.path.dirname(path)
        if parent:
            os.makedirs(parent, exist_ok=True)
        with open(path, "w", encoding="utf-8") as f:
            f.write(content)
        if name.endswith((".c", ".s", ".S")):
            sources.append(path)
    sources.extend(support_sources)

    if linker_script:
        artifact = os.path.join(build_dir, "firmware.elf")
        cmd = [cc] + cpu_flags + extra_flags + ["-T", linker_script] + sources + ["-o", artifact]
    else:
        artifact = os.path.join(build_dir, "out.o")
        cmd = [cc] + cpu_flags + extra_flags + sources + ["-o", artifact]

    try:
        proc = subprocess.run(cmd, capture_output=True, text=True)
    except FileNotFoundError:
        return {"ok": False, "returncode": -1, "stdout": "", "stderr": "",
                "artifact_path": None, "toolchain": f"{cc} (absent)", "available": False}

    ok = proc.returncode == 0 and os.path.exists(artifact)

    version = cc
    try:
        v = subprocess.run([cc, "--version"], capture_output=True, text=True)
        if v.stdout:
            version = v.stdout.splitlines()[0]
    except Exception:  # noqa: BLE001
        pass

    return {
        "ok": ok,
        "returncode": proc.returncode,
        "stdout": proc.stdout,
        "stderr": proc.stderr,
        "artifact_path": artifact if ok else None,
        "toolchain": version,
        "available": True,
    }


TOOLS = {
    "compile_firmware": {
        "fn": compile_firmware,
        "desc": "Compile+link firmware with a native GCC toolchain; returns raw build results.",
        "schema": {
            "type": "object",
            "properties": {
                "cc": {"type": "string"},
                "cpu_flags": {"type": "array", "items": {"type": "string"}},
                "extra_flags": {"type": "array", "items": {"type": "string"}},
                "support_sources": {"type": "array", "items": {"type": "string"}},
                "linker_script": {"type": ["string", "null"]},
                "target": {"type": "string"},
                "entry": {"type": "string"},
                "files": {"type": "object"},
            },
            "required": ["files"],
        },
    },
}


if __name__ == "__main__":
    serve("embeder-firmware", TOOLS)
