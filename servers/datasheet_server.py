"""Embeder Datasheet MCP server (ADR-0001, ADR-0002).

Exposes `register_map` / `lookup_register` over MCP, sourced from a CMSIS-SVD file.
A hit returns absolute addresses + a source; a miss returns found=False and is
refused, never fabricated. (The Rust `datasheet` crate is the reference parser; this
server reads the same SVD file, and a cross-check test guards against drift.)
"""
import os
import sys
import xml.etree.ElementTree as ET

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from mcp_lib import serve  # noqa: E402


def _int(s):
    s = (s or "0").strip()
    return int(s, 16) if s.lower().startswith("0x") else int(s)


def _parse(svd_path):
    root = ET.parse(svd_path).getroot()
    peris = {}
    order = []
    for p in root.iter("peripheral"):
        name = p.findtext("name")
        base = _int(p.findtext("baseAddress") or "0")
        derived = p.get("derivedFrom")
        regs = []
        rsroot = p.find("registers")
        if rsroot is not None:
            for r in rsroot.findall("register"):
                fields = []
                froot = r.find("fields")
                if froot is not None:
                    for f in froot.findall("field"):
                        fields.append({
                            "name": f.findtext("name"),
                            "bit_offset": _int(f.findtext("bitOffset") or "0"),
                            "bit_width": _int(f.findtext("bitWidth") or "1"),
                            "description": f.findtext("description"),
                        })
                regs.append({
                    "name": r.findtext("name"),
                    "offset": _int(r.findtext("addressOffset") or "0"),
                    "description": r.findtext("description"),
                    "fields": fields,
                })
        peris[name.lower()] = {"name": name, "base": base, "regs": regs, "derived": derived}
        order.append(name.lower())

    # resolve derivedFrom
    for k in order:
        pp = peris[k]
        if not pp["regs"] and pp["derived"]:
            src = peris.get(pp["derived"].lower())
            if src:
                pp["regs"] = src["regs"]
    return peris


def register_map(svd_path, peripheral):
    peris = _parse(svd_path)
    p = peris.get(peripheral.lower())
    if not p:
        return {"found": False,
                "note": f"peripheral '{peripheral}' not in structured source; not fabricated"}
    registers = [{
        "name": r["name"],
        "offset": r["offset"],
        "absolute_address": p["base"] + r["offset"],
        "description": r["description"],
        "fields": r["fields"],
    } for r in p["regs"]]
    return {"found": True, "name": p["name"], "base_address": p["base"],
            "registers": registers, "source": os.path.basename(svd_path)}


def lookup_register(svd_path, peripheral, register):
    rm = register_map(svd_path, peripheral)
    if not rm.get("found"):
        return rm
    for r in rm["registers"]:
        if r["name"].lower() == register.lower():
            return {"found": True, "name": rm["name"], "register": r, "source": rm["source"]}
    return {"found": False,
            "note": f"register '{peripheral}.{register}' not in structured source; not fabricated"}


_SCHEMA = {
    "type": "object",
    "properties": {
        "svd_path": {"type": "string"},
        "peripheral": {"type": "string"},
        "register": {"type": "string"},
    },
    "required": ["svd_path", "peripheral"],
}

TOOLS = {
    "register_map": {"fn": register_map, "desc": "Grounded register map for a peripheral.", "schema": _SCHEMA},
    "lookup_register": {"fn": lookup_register, "desc": "Grounded single register lookup.", "schema": _SCHEMA},
}


if __name__ == "__main__":
    serve("embeder-datasheet", TOOLS)
