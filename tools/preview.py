#!/usr/bin/env python3
"""
VOID // UI preview server  (development aid — not the product)

The product is the Rust binary in ../src (`cargo run --release`), which serves
this exact UI and applies real tweaks.

This script exists so the interface can be reviewed in a browser without a Rust
toolchain. It reads the *real* catalog straight out of src/engine/catalog.rs, so
the tweak list can never drift from the engine, and answers the same JSON API.
Every system mutation is simulated. Nothing on your machine is touched.
"""

import json
import os
import random
import re
import socket
import sys
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import urlparse, parse_qs
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
UI = ROOT / "assets" / "ui"
CATALOG_RS = ROOT / "src" / "engine" / "catalog.rs"
VERSION = "0.1.0"


# ─────────────────────────── catalog parsing ───────────────────────────

def _block(text, start):
    """Return the balanced {...} block starting at index `start` (a '{')."""
    depth, i, in_str = 0, start, False
    while i < len(text):
        ch = text[i]
        if in_str:
            if ch == "\\":
                i += 2
                continue
            if ch == '"':
                in_str = False
        else:
            if ch == '"':
                in_str = True
            elif ch == "{":
                depth += 1
            elif ch == "}":
                depth -= 1
                if depth == 0:
                    return text[start:i + 1]
        i += 1
    return text[start:]


def _blocks(text, opener):
    out, i = [], 0
    while True:
        j = text.find(opener, i)
        if j < 0:
            return out
        brace = text.find("{", j)
        blk = _block(text, brace)
        out.append(blk)
        i = brace + len(blk)


def _bracket(text, opener):
    j = text.find(opener)
    if j < 0:
        return ""
    i = text.find("[", j)
    depth, k, in_str = 0, i, False
    while k < len(text):
        ch = text[k]
        if in_str:
            if ch == "\\":
                k += 2
                continue
            if ch == '"':
                in_str = False
        else:
            if ch == '"':
                in_str = True
            elif ch == "[":
                depth += 1
            elif ch == "]":
                depth -= 1
                if depth == 0:
                    return text[i + 1:k]
        k += 1
    return ""


def _field(blk, name, default=""):
    m = re.search(r'\b%s:\s*"((?:[^"\\]|\\.)*)"' % name, blk)
    return m.group(1).replace('\\"', '"').replace("\\\\", "\\") if m else default


def _flag(blk, name):
    return bool(re.search(r"\b%s:\s*true" % name, blk))


def _int(blk, name, default=0):
    m = re.search(r"\b%s:\s*(\d+)" % name, blk)
    return int(m.group(1)) if m else default


def _action_text(a):
    a = a.strip()
    if a.startswith("Special("):
        m = re.match(r'Special\("([^"]*)"\)', a)
        return "void::" + (m.group(1) if m else "?")
    if a.startswith("RegDel("):
        m = re.match(r'RegDel\(\s*"([^"]*)",\s*"([^"]*)"\s*,?\s*\)', a)
        return 'reg delete "%s" /v "%s" /f' % tuple(x.replace("\\\\", "\\") for x in m.groups()) if m else a
    if a.startswith("Reg("):
        m = re.match(r'Reg\(\s*"([^"]*)",\s*"([^"]*)",\s*"([^"]*)",\s*"([^"]*)"\s*,?\s*\)', a)
        return 'reg add "%s" /v "%s" /t %s /d %s /f' % tuple(x.replace("\\\\", "\\") for x in m.groups()) if m else a
    if a.startswith("SvcEnable("):
        m = re.match(r'SvcEnable\("([^"]*)"\)', a)
        return "sc config %s start= auto  ·  sc start %s" % (m.group(1), m.group(1)) if m else a
    if a.startswith("Svc("):
        m = re.match(r'Svc\("([^"]*)"\)', a)
        return "sc config %s start= disabled  ·  sc stop %s" % (m.group(1), m.group(1)) if m else a
    if a.startswith("Cmd("):
        m = re.match(r'Cmd\(\s*"([^"]*)",\s*&\[(.*)\]\s*,?\s*\)', a, re.S)
        if m:
            args = re.findall(r'"([^"]*)"', m.group(2))
            return " ".join([m.group(1)] + args)
        return a
    return a


def _actions(blk, key):
    inner = _bracket(blk, key + ":")
    if not inner:
        return []
    return [_action_text(p) for p in re.split(r"Action::", inner)[1:] if p.strip()]


def parse_catalog():
    src = CATALOG_RS.read_text(encoding="utf-8")
    tweaks = []
    for blk in _blocks(src, "Tweak {"):
        if 'id: "' not in blk:
            continue
        tweaks.append({
            "id": _field(blk, "id"),
            "name": _field(blk, "name"),
            "cat": _field(blk, "cat"),
            "desc": _field(blk, "desc"),
            "why": _field(blk, "why"),
            "risk": _int(blk, "risk"),
            "reboot": _flag(blk, "reboot"),
            "once": _flag(blk, "once"),
            "plan": _actions(blk, "apply"),
            "revplan": _actions(blk, "revert"),
        })
    profiles = []
    for blk in _blocks(src, "Profile {"):
        if 'id: "' not in blk:
            continue
        inner = _bracket(blk, "tweaks:")
        ids = re.findall(r'"([^"]+)"', inner)
        profiles.append({
            "id": _field(blk, "id"),
            "name": _field(blk, "name"),
            "tagline": _field(blk, "tagline"),
            "note": _field(blk, "note"),
            "count": len(ids),
            "tweaks": ids,
        })
    mcat = re.search(r"CATEGORIES:[^=]*=\s*&\[(.*?)\];", src, re.S)
    cats = [{"id": a, "label": b} for a, b in
            re.findall(r'\("([^"]+)",\s*"([^"]+)"\)', mcat.group(1) if mcat else "")]
    return {"tweaks": tweaks, "profiles": profiles, "categories": cats, "version": VERSION}


CATALOG = parse_catalog()
TWEAK_BY_ID = {t["id"]: t for t in CATALOG["tweaks"]}
PROFILE_BY_ID = {p["id"]: p for p in CATALOG["profiles"]}


# ─────────────────────────── machine + state ───────────────────────────

def cpu_name():
    try:
        for line in Path("/proc/cpuinfo").read_text().splitlines():
            if line.startswith("model name"):
                return line.split(":", 1)[1].strip()
    except Exception:
        pass
    return "Unknown CPU"


def os_name():
    try:
        for line in Path("/etc/os-release").read_text().splitlines():
            if line.startswith("PRETTY_NAME="):
                return line.split("=", 1)[1].strip().strip('"')
    except Exception:
        pass
    return "Linux"


def uptime_secs():
    try:
        return int(float(Path("/proc/uptime").read_text().split()[0]))
    except Exception:
        return 0


MACHINE = {
    "os": os_name() + " · preview",
    "host": socket.gethostname(),
    "user": os.environ.get("USER") or os.environ.get("USERNAME") or "player",
    "cpu": cpu_name(),
    "gpu": "preview renderer",
    "admin": os.geteuid() == 0 if hasattr(os, "geteuid") else False,
    "simulated": True,
}

STATE = {
    "dry_run": False,
    "applied": [],
    "log": [],
    "volume": 34,
    "muted": False,
    "motion": True,
    "auto_backup": True,
    "last_profile": "",
    "started": int(time.time()),
    "ops": 0,
}
LOCK = threading.Lock()


def push(level, msg):
    STATE["log"].append({
        "at": time.strftime("%H:%M:%S", time.gmtime()),
        "level": level,
        "msg": msg,
    })
    del STATE["log"][:-500]
    try:
        with open(ROOT / "data" / "preview.log", "a", encoding="utf-8") as fh:
            fh.write("%s [%s] %s\n" % (time.strftime("%Y-%m-%d %H:%M:%S"), level, msg))
    except Exception:
        pass


def state_json():
    return {
        "version": VERSION,
        "os": MACHINE["os"],
        "host": MACHINE["host"],
        "user": MACHINE["user"],
        "cpu": MACHINE["cpu"],
        "gpu": MACHINE["gpu"],
        "admin": MACHINE["admin"],
        "simulated": True,
        "dry_run": STATE["dry_run"],
        "applied": STATE["applied"],
        "settings": {
            "volume": STATE["volume"],
            "muted": STATE["muted"],
            "motion": STATE["motion"],
            "auto_backup": STATE["auto_backup"],
            "last_profile": STATE["last_profile"],
        },
        "uptime": int(time.time()) - STATE["started"],
        "ops": STATE["ops"],
        "total_tweaks": len(CATALOG["tweaks"]),
        "applied_count": len(STATE["applied"]),
        "now": time.strftime("%Y-%m-%d %H:%M:%S", time.gmtime()),
    }


def catalog_json():
    applied = set(STATE["applied"])
    out = dict(CATALOG)
    out["tweaks"] = [dict(t, applied=t["id"] in applied) for t in CATALOG["tweaks"]]
    return out


def apply_one(t, revert):
    steps = t["revplan"] if revert else t["plan"]
    msgs = []
    ok = True
    for step in steps:
        time.sleep(0.035)
        msgs.append("simulated :: " + step)
    if not steps:
        msgs.append("no action required")
    key = t["id"]
    if not revert:
        if t["once"]:
            if key in STATE["applied"]:
                STATE["applied"].remove(key)
        elif key not in STATE["applied"]:
            STATE["applied"].append(key)
        STATE["ops"] += 1
    else:
        if key in STATE["applied"]:
            STATE["applied"].remove(key)
    push("ok" if ok else "err", "%s :: %s" % (t["name"], "  ·  ".join(msgs)))
    return {"id": key, "ok": ok, "msg": "  ·  ".join(msgs)}


def apply_ids(ids):
    results = [apply_one(TWEAK_BY_ID[i], False) for i in ids if i in TWEAK_BY_ID]
    missing = [i for i in ids if i not in TWEAK_BY_ID]
    for m in missing:
        results.append({"id": m, "ok": False, "msg": "unknown tweak"})
    return results


def read_cpu_sample():
    try:
        line = Path("/proc/stat").read_text().splitlines()[0]
        vals = [int(v) for v in line.split()[1:]]
        idle = vals[3] + (vals[4] if len(vals) > 4 else 0)
        return idle, sum(vals)
    except Exception:
        return None


def stats_json():
    a = read_cpu_sample()
    time.sleep(0.12)
    b = read_cpu_sample()
    cpu = 0.0
    if a and b:
        dt = b[1] - a[1]
        di = b[0] - a[0]
        if dt > 0:
            cpu = max(0.0, min(100.0, (dt - di) / dt * 100.0))
    if cpu == 0.0:
        # Some sandboxes expose a frozen /proc/stat. Keep the preview honest but
        # alive: this is a synthetic sample, used only by the preview server.
        t = time.time()
        cpu = 14.0 + 9.0 * (0.5 + 0.5 * __import__("math").sin(t / 3.1)) + random.random() * 4.0
    total = used = 0.0
    try:
        for line in Path("/proc/meminfo").read_text().splitlines():
            if line.startswith("MemTotal:"):
                total = float(line.split()[1]) / 1024 / 1024
            if line.startswith("MemAvailable:"):
                avail = float(line.split()[1]) / 1024 / 1024
                used = max(0.0, total - avail)
    except Exception:
        pass
    return {
        "cpu": round(cpu, 2),
        "ram": round(used / total * 100, 2) if total else 0.0,
        "ram_total": round(total, 2),
        "ram_used": round(used, 2),
        "uptime": uptime_secs(),
        "ts": int(time.time()),
    }


def bench_json():
    samples = []
    for _ in range(120):
        t0 = time.perf_counter()
        time.sleep(0.001)
        samples.append((time.perf_counter() - t0) * 1000.0)
    samples.sort()
    avg = sum(samples) / len(samples)
    p95 = samples[int(len(samples) * 0.95) % len(samples)]
    jitter = max(0.0, avg - 1.0)
    return {
        "avg_ms": round(avg, 3),
        "p50_ms": round(samples[len(samples) // 2], 3),
        "p95_ms": round(p95, 3),
        "max_ms": round(samples[-1], 3),
        "jitter_ms": round(jitter, 3),
        "score": round(max(0.0, min(100.0, 100.0 - jitter * 6.0)), 1),
        "samples": len(samples),
        "ts": int(time.time()),
    }


# ─────────────────────────── http ───────────────────────────

MIME = {".html": "text/html; charset=utf-8", ".css": "text/css; charset=utf-8",
        ".js": "application/javascript; charset=utf-8", ".svg": "image/svg+xml",
        ".json": "application/json"}

FAVICON = ("<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 32 32'>"
           "<rect width='32' height='32' fill='#000'/>"
           "<path d='M7 8 L16 25 L25 8' fill='none' stroke='#fff' stroke-width='2.2'/></svg>")


class Handler(BaseHTTPRequestHandler):
    server_version = "VOID-preview/" + VERSION

    def log_message(self, fmt, *args):
        # keep the console quiet unless something actually went wrong
        if args and str(args[1]).startswith(("4", "5")):
            sys.stderr.write("  %s\n" % (fmt % args))

    def send(self, status, ctype, body):
        if isinstance(body, str):
            body = body.encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", ctype)
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Cache-Control", "no-store")
        self.end_headers()
        try:
            self.wfile.write(body)
        except BrokenPipeError:
            pass

    def js(self, obj):
        self.send(200, "application/json; charset=utf-8", json.dumps(obj))

    def static(self, rel):
        path = (UI / rel).resolve()
        if not str(path).startswith(str(UI.resolve())) or not path.is_file():
            return self.send(404, "text/plain", "not found")
        self.send(200, MIME.get(path.suffix, "application/octet-stream"), path.read_bytes())

    def do_GET(self):
        u = urlparse(self.path)
        p = u.path
        if p in ("/", "/index.html"):
            return self.static("index.html")
        if p in ("/favicon.ico", "/favicon.svg"):
            return self.send(200, "image/svg+xml", FAVICON)
        if p.startswith("/assets/"):
            return self.static(p[len("/assets/"):])
        if p == "/api/catalog":
            with LOCK:
                return self.js(catalog_json())
        if p == "/api/state":
            with LOCK:
                return self.js(state_json())
        if p == "/api/log":
            with LOCK:
                return self.js(STATE["log"])
        if p == "/api/stats":
            return self.js(stats_json())
        if p == "/api/bench":
            return self.js(bench_json())
        return self.send(404, "text/plain", "not found")

    def do_POST(self):
        u = urlparse(self.path)
        q = parse_qs(u.query)
        one = lambda k: (q.get(k) or [""])[0]
        p = u.path
        with LOCK:
            if p == "/api/apply":
                tid = one("id")
                if not tid:
                    return self.send(400, "text/plain", "missing id")
                return self.js({"results": apply_ids([tid])})
            if p == "/api/profile":
                pid = one("id")
                prof = PROFILE_BY_ID.get(pid)
                if not prof:
                    return self.js({"results": [{"id": pid, "ok": False, "msg": "unknown profile"}]})
                STATE["last_profile"] = pid
                push("sys", "profile engaged :: " + prof["name"])
                return self.js({"results": apply_ids(prof["tweaks"]),
                                "applied_count": len(STATE["applied"])})
            if p == "/api/revert":
                tid = one("id")
                if tid and tid in TWEAK_BY_ID:
                    return self.js({"results": [apply_one(TWEAK_BY_ID[tid], True)],
                                    "applied_count": len(STATE["applied"])})
                ids = list(STATE["applied"])
                res = [apply_one(TWEAK_BY_ID[i], True) for i in ids if i in TWEAK_BY_ID] or \
                      [{"id": "*", "ok": True, "msg": "nothing to revert"}]
                STATE["applied"] = []
                return self.js({"results": res, "applied_count": 0})
            if p == "/api/revert-all":
                ids = list(STATE["applied"])
                res = [apply_one(TWEAK_BY_ID[i], True) for i in ids if i in TWEAK_BY_ID] or \
                      [{"id": "*", "ok": True, "msg": "nothing to revert"}]
                STATE["applied"] = []
                return self.js({"results": res, "applied_count": 0})
            if p == "/api/restore":
                time.sleep(0.4)
                push("ok", "restore point :: simulated :: Checkpoint-Computer -Description 'VOID restore point'")
                return self.js({"results": [{"id": "sys.restore", "ok": True,
                                             "msg": "simulated :: restore point created"}]})
            if p == "/api/setting":
                key, value = one("key"), one("value")
                if key == "volume":
                    STATE["volume"] = max(0, min(100, int(value or 0)))
                elif key == "muted":
                    STATE["muted"] = value in ("1", "true", "on", "yes")
                elif key == "motion":
                    STATE["motion"] = value in ("1", "true", "on", "yes")
                elif key == "auto_backup":
                    STATE["auto_backup"] = value in ("1", "true", "on", "yes")
                elif key == "dry_run":
                    STATE["dry_run"] = value in ("1", "true", "on", "yes")
                return self.js({"ok": True})
            if p == "/api/shutdown":
                self.send(200, "text/plain", "bye")
                threading.Thread(target=self.server.shutdown, daemon=True).start()
                return None
        return self.send(404, "text/plain", "not found")


def main():
    port = 8787
    host = "0.0.0.0"
    for i, a in enumerate(sys.argv):
        if a == "--port" and i + 1 < len(sys.argv):
            port = int(sys.argv[i + 1])
        if a == "--host" and i + 1 < len(sys.argv):
            host = sys.argv[i + 1]
    push("sys", "engine online · preview mode · %s" % MACHINE["os"])
    srv = ThreadingHTTPServer((host, port), Handler)
    print("\n  V O I D   //   preview   v%s" % VERSION)
    print("  ─────────────────────────────────────────────")
    print("  catalog  %d tweaks · %d profiles (parsed from src/engine/catalog.rs)"
          % (len(CATALOG["tweaks"]), len(CATALOG["profiles"])))
    print("  mode     SIMULATION — nothing is written to your system")
    print("  url      http://%s:%d/" % ("127.0.0.1" if host == "0.0.0.0" else host, port))
    print("  ─────────────────────────────────────────────")
    print("  the real engine:  cargo run --release\n")
    try:
        srv.serve_forever()
    except KeyboardInterrupt:
        print("\n  stopped.\n")


if __name__ == "__main__":
    main()
