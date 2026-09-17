"""Embeder Codegen MCP server (ADR-0001) — the BYOK model in the loop.

Exposes `generate_code` over MCP: given a system + user prompt (built by the Rust
Core from grounded datasheet facts and any prior compiler diagnostics), call a
Bring-Your-Own-Key LLM and return the C source it drafted. The model only ever
*proposes*; the compile + simulation oracles decide whether the draft is VERIFIED.

Key handling is deliberately boring and local: keys come from the environment or a
`.env` file, never from arguments, and never leave this process except to the
provider endpoint. No key configured -> `available: false`, and the Core falls back
to deterministic grounded synthesis so the loop still closes.

Providers (auto-detected from whichever key is present, or forced with
`EMBEDER_LLM_PROVIDER` / the `provider` arg):
  * gemini  — Google Generative Language API      (GEMINI_API_KEY / GOOGLE_API_KEY)
  * vercel  — Vercel AI Gateway (OpenAI-compatible) (AI_GATEWAY_API_KEY)
  * openai  — OpenAI or any OpenAI-compatible API  (OPENAI_API_KEY, OPENAI_BASE_URL)
Generic overrides for any OpenAI-compatible endpoint:
  EMBEDER_LLM_BASE_URL, EMBEDER_LLM_API_KEY, EMBEDER_LLM_MODEL.

Pure stdlib (urllib for HTTP), so it stays a drop-in MCP server with no deps.
"""
import os
import re
import sys
import json
import urllib.request
import urllib.error

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from mcp_lib import serve  # noqa: E402

REPO_ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
# Providers explicitly turned off (used by tests and offline runs) resolve to "no LLM".
DISABLED = {"none", "off", "disabled", "__none__"}


def _load_dotenv():
    """Populate os.environ from local .env files without overriding real env vars."""
    for rel in (".env", ".env.local", os.path.join("desktop", ".env.local")):
        path = os.path.join(REPO_ROOT, rel)
        try:
            with open(path, encoding="utf-8") as f:
                for raw in f:
                    line = raw.strip()
                    if not line or line.startswith("#") or "=" not in line:
                        continue
                    key, value = line.split("=", 1)
                    key = key.strip()
                    value = value.strip().strip('"').strip("'")
                    if key and key not in os.environ:
                        os.environ[key] = value
        except OSError:
            pass


def _resolve_provider(provider):
    p = (provider or os.environ.get("EMBEDER_LLM_PROVIDER") or "").strip().lower()
    if p in DISABLED:
        return None
    if p:
        return p
    # Auto-detect from whichever key is present.
    if os.environ.get("EMBEDER_LLM_API_KEY") and os.environ.get("EMBEDER_LLM_BASE_URL"):
        return "openai"
    if os.environ.get("GEMINI_API_KEY") or os.environ.get("GEMINI_AI_KEY") or os.environ.get("GOOGLE_API_KEY"):
        return "gemini"
    if os.environ.get("AI_GATEWAY_API_KEY") or os.environ.get("VERCEL_AI_GATEWAY_API_KEY"):
        return "vercel"
    if os.environ.get("OPENAI_API_KEY"):
        return "openai"
    return None


def _provider_config(provider):
    """Resolve endpoint + key + model for a provider, or None if unknown."""
    model_override = os.environ.get("EMBEDER_LLM_MODEL")
    key_override = os.environ.get("EMBEDER_LLM_API_KEY")
    base_override = os.environ.get("EMBEDER_LLM_BASE_URL")

    if provider in ("gemini", "google"):
        return {
            "kind": "gemini",
            "base_url": base_override or "https://generativelanguage.googleapis.com/v1beta",
            "api_key": key_override or os.environ.get("GEMINI_API_KEY") or os.environ.get("GEMINI_AI_KEY") or os.environ.get("GOOGLE_API_KEY"),
            "model": model_override or "gemini-3.6-flash",
        }
    if provider in ("vercel", "vercel-ai-gateway", "ai-gateway"):
        return {
            "kind": "openai",
            "base_url": base_override or "https://ai-gateway.vercel.sh/v1",
            "api_key": key_override or os.environ.get("AI_GATEWAY_API_KEY") or os.environ.get("VERCEL_AI_GATEWAY_API_KEY"),
            "model": model_override or "openai/gpt-4o-mini",
        }
    if provider in ("openai", "openai-compatible"):
        return {
            "kind": "openai",
            "base_url": base_override or os.environ.get("OPENAI_BASE_URL") or "https://api.openai.com/v1",
            "api_key": key_override or os.environ.get("OPENAI_API_KEY"),
            "model": model_override or "gpt-4o-mini",
        }
    return None


def _http_post_json(url, headers, payload, timeout):
    data = json.dumps(payload).encode("utf-8")
    req = urllib.request.Request(url, data=data, headers=headers, method="POST")
    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            return json.loads(resp.read().decode("utf-8"))
    except urllib.error.HTTPError as e:
        body = ""
        try:
            body = e.read().decode("utf-8", errors="replace")[:500]
        except Exception:  # noqa: BLE001
            pass
        raise RuntimeError(f"HTTP {e.code}: {body or e.reason}")


def _call_gemini(cfg, system, user, temperature, max_tokens, timeout):
    url = f"{cfg['base_url']}/models/{cfg['model']}:generateContent?key={cfg['api_key']}"
    payload = {
        "system_instruction": {"parts": [{"text": system}]},
        "contents": [{"role": "user", "parts": [{"text": user}]}],
        "generationConfig": {"temperature": temperature, "maxOutputTokens": max_tokens},
    }
    resp = _http_post_json(url, {"Content-Type": "application/json"}, payload, timeout)
    candidates = resp.get("candidates") or []
    if not candidates:
        raise RuntimeError(f"empty Gemini response: {json.dumps(resp)[:300]}")
    parts = candidates[0].get("content", {}).get("parts") or []
    return "".join(p.get("text", "") for p in parts)


def _call_openai(cfg, system, user, temperature, max_tokens, timeout):
    url = f"{cfg['base_url']}/chat/completions"
    headers = {"Content-Type": "application/json", "Authorization": f"Bearer {cfg['api_key']}"}
    payload = {
        "model": cfg["model"],
        "messages": [
            {"role": "system", "content": system},
            {"role": "user", "content": user},
        ],
        "temperature": temperature,
        "max_tokens": max_tokens,
    }
    resp = _http_post_json(url, headers, payload, timeout)
    choices = resp.get("choices") or []
    if not choices:
        raise RuntimeError(f"empty completion: {json.dumps(resp)[:300]}")
    return choices[0].get("message", {}).get("content", "") or ""


_FENCE = re.compile(r"```(?:c|cpp|c\+\+|arm|objc)?\s*\n(.*?)```", re.DOTALL | re.IGNORECASE)


def _extract_code(text):
    """Pull the C source out of a model reply — a fenced block if present, else the
    whole thing when it already looks like C."""
    if not text:
        return ""
    m = _FENCE.search(text)
    if m:
        return m.group(1).strip() + "\n"
    stripped = text.strip()
    return stripped + "\n" if stripped else ""


def generate_code(system="", user="", provider=None, model=None,
                  temperature=0.2, max_tokens=8192, timeout_secs=60):
    _load_dotenv()
    try:
        temperature = float(temperature)
    except (TypeError, ValueError):
        temperature = 0.2
    max_tokens = max(256, min(int(max_tokens), 16384))
    timeout_secs = max(5, min(int(timeout_secs), 180))

    resolved = _resolve_provider(provider)
    if resolved is None:
        return {"available": False, "provider": None, "model": None, "code": "", "text": "",
                "error": "no LLM configured (set GEMINI_API_KEY, AI_GATEWAY_API_KEY or OPENAI_API_KEY)"}

    cfg = _provider_config(resolved)
    if cfg is None:
        return {"available": False, "provider": resolved, "model": None, "code": "", "text": "",
                "error": f"unknown provider: {resolved}"}
    if model:
        cfg["model"] = model
    if not cfg["api_key"]:
        return {"available": False, "provider": resolved, "model": cfg["model"], "code": "", "text": "",
                "error": f"no API key for provider '{resolved}'"}

    try:
        if cfg["kind"] == "gemini":
            text = _call_gemini(cfg, system, user, temperature, max_tokens, timeout_secs)
        else:
            text = _call_openai(cfg, system, user, temperature, max_tokens, timeout_secs)
    except Exception as e:  # noqa: BLE001 - report any provider/network failure, never crash the loop
        return {"available": True, "provider": resolved, "model": cfg["model"], "code": "", "text": "",
                "error": f"{type(e).__name__}: {e}"}

    return {"available": True, "provider": resolved, "model": cfg["model"],
            "code": _extract_code(text), "text": text, "error": None}


TOOLS = {
    "generate_code": {
        "fn": generate_code,
        "desc": "Draft firmware C source with a BYOK LLM from a grounded prompt; returns source only.",
        "schema": {
            "type": "object",
            "properties": {
                "system": {"type": "string"},
                "user": {"type": "string"},
                "provider": {"type": ["string", "null"]},
                "model": {"type": ["string", "null"]},
                "temperature": {"type": "number"},
                "max_tokens": {"type": "integer"},
                "timeout_secs": {"type": "integer"},
            },
            "required": ["system", "user"],
        },
    },
}


if __name__ == "__main__":
    serve("embeder-codegen", TOOLS)
