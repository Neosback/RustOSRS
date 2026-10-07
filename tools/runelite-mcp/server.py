#!/usr/bin/env python3
"""runelite-render-mcp — dependency-free MCP server for RuneLite render reference.

Serves the workspace's own reference material over MCP stdio (JSON-RPC 2.0,
newline-delimited), so any MCP-capable agent/IDE can query it:
  * runelite-api sources (local Oct-2026 snapshot): class/method/field lookup
  * staged shaders in reference-shaders/runelite-gpu
  * the RUNELITE_*.md guides in the workspace root

Stdlib only (json, sys, os, re). No network use at runtime; apidocs URLs are
included as links only.

Run:  python3 server.py
Test: echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' | python3 server.py
"""
import json
import os
import re
import sys

WORKSPACE = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
API_SRC = "/Users/tylercovalt/Documents/runelite-master/runelite-api/src/main/java"
GPU_JAVA = ("/Users/tylercovalt/Documents/runelite-master"
            "/runelite-client/src/main/java/net/runelite/client/plugins/gpu")
_DEOB = "/Users/tylercovalt/Documents/ChatGPT/RSPSi-resources/RuneLite-melxin/runescape-client/src/main/java"
DEOB_FILES = ["Scene.java", "SceneTileModel.java", "SceneTilePaint.java",
              "ObjectComposition.java", "Model.java", "ModelData.java",
              "FloorUnderlayDefinition.java", "FloorOverlayDefinition.java",
              "Tiles.java", "Tile.java", "BoundaryObject.java",
              "WallDecoration.java", "FloorDecoration.java",
              "class150.java", "class470.java"]
SHADERS = os.path.join(WORKSPACE, "reference-shaders", "runelite-gpu")
APIDOCS = "https://static.runelite.net/runelite-api/apidocs"
PROTOCOL_VERSION = "2024-11-05"

_modifiers = r"(?:@\w+(?:\([^)]*\))?\s*|public\s+|protected\s+|private\s+|static\s+|final\s+|abstract\s+|default\s+|synchronized\s+|native\s+|strictfp\s+|transient\s+|volatile\s+)*"
_type_tok = r"[\w<>\[\]., ?]+?"

_javadoc_re = re.compile(r"/\*\*(.*?)\*/", re.S)
_type_re = re.compile(
    r"^[ \t]*(public[ \t]+)?(final[ \t]+)?(abstract[ \t]+)?(interface|class|enum)[ \t]+(\w+)([^\{;]*)\{?",
    re.M,
)
_mod_word = r"(?:public|protected|private|static|final|abstract|default|synchronized|native|strictfp|transient|volatile)"
_sig_name_re = re.compile(r"^\s*(?:%s\s+)*([\w<>\[\]., ?]+?)\s+([A-Za-z_]\w*)\s*\(" % _mod_word)
_field_re = re.compile(
    r"^\s*(?:%s\s+)*([\w<>\[\].]+)\s+([A-Za-z_]\w*(?:\s*,\s*[A-Za-z_]\w*)*)\s*(=[^;]*)?;\s*$" % _mod_word
)

JAVA_KEYWORDS = {
    "if", "for", "while", "switch", "catch", "return", "new", "throw",
    "assert", "synchronized",
}


def _clean_doc(raw):
    lines = []
    for ln in raw.split("\n"):
        ln = ln.strip().lstrip("*").strip()
        if ln.startswith("@"):
            break
        lines.append(ln)
    text = " ".join(w for w in " ".join(lines).split() if w)
    text = re.sub(r"\{@(\w+)\s+([^}]*)\}", r"\2", text)
    m = re.search(r"\.\s+[A-Z]", text)
    return text[: m.start() + 1] if m else text[:400]


def _attach_docs(src):
    """Return list of (end_pos, cleaned_doc) for javadoc blocks."""
    out = []
    for m in _javadoc_re.finditer(src):
        out.append((m.end(), _clean_doc(m.group(1))))
    return out


def _doc_before(docs, pos):
    best = ""
    for end, doc in docs:
        if 0 <= pos - end < 400:
            best = doc
    return best


def _strip_line(s):
    s = s.strip()
    if s.startswith("@"):
        return ""
    return s


def _code_braces(s, state):
    """Count net braces ignoring comments/strings. state={'block': bool} persists."""
    i, n, net = 0, len(s), 0
    while i < n:
        if state["block"]:
            j = s.find("*/", i)
            if j < 0:
                return net
            i = j + 2
            state["block"] = False
            continue
        c = s[i]
        if c == "/" and i + 1 < n and s[i + 1] == "/":
            break
        if c == "/" and i + 1 < n and s[i + 1] == "*":
            state["block"] = True
            i += 2
            continue
        if c in "\"'":
            q = c
            i += 1
            while i < n and s[i] != q:
                i += 2 if s[i] == "\\" else 1
            i += 1
            continue
        if c == "{":
            net += 1
        elif c == "}":
            net -= 1
        i += 1
    return net


def scan_types(path):
    """Fast name-only scan for index building (no member parsing)."""
    try:
        with open(path, "r", encoding="utf-8", errors="replace") as f:
            src = f.read()
    except OSError:
        return None
    pkg = ""
    m = re.search(r"^\s*package\s+([\w.]+)\s*;", src, re.M)
    if m:
        pkg = m.group(1)
    types = []
    for tm in _type_re.finditer(src):
        extra = " ".join((tm.group(6) or "").split())
        types.append({"kind": tm.group(4), "name": tm.group(5),
                      "decl": ("%s %s %s" % (tm.group(4), tm.group(5), extra)).strip()})
    return {"package": pkg, "path": path, "types": types}


def parse_java(path):
    with open(path, "r", encoding="utf-8", errors="replace") as f:
        src = f.read()
    pkg = ""
    m = re.search(r"^\s*package\s+([\w.]+)\s*;", src, re.M)
    if m:
        pkg = m.group(1)
    docs = _attach_docs(src)
    types = []
    for tm in _type_re.finditer(src):
        kind, name, extra = tm.group(4), tm.group(5), (tm.group(6) or "").strip()
        extra = " ".join(extra.split())
        if len(extra) > 220:
            extra = extra[:220] + "..."
        types.append({
            "kind": kind, "name": name,
            "decl": ("%s %s %s" % (kind, name, extra)).strip(),
            "doc": _doc_before(docs, tm.start()),
            "pos": tm.start(),
        })
    if not types:
        return {"package": pkg, "path": path, "types": []}
    main = types[0]
    # Line-based member scan (linear time): join continued lines up to a
    # balanced close-paren, then classify as method or field.
    lines = src.split("\n")
    pos = 0
    line_starts = []
    for ln in lines:
        line_starts.append(pos)
        pos += len(ln) + 1
    pending_doc, in_doc = "", False
    methods, fields, seen = [], [], set()
    buf, buf_start, depth = "", 0, 0
    brace, buf_brace = 0, 0
    cstate = {"block": False}
    i = 0
    n = len(lines)
    while i < n:
        raw = lines[i]
        s = raw.strip()
        if s.startswith("/**"):
            in_doc = True
            pending_doc = ""
        if in_doc:
            pending_doc += s.strip().lstrip("/*").strip() + "\n"
            if "*/" in s:
                in_doc = False
            brace += _code_braces(raw, cstate)
            i += 1
            continue
        code = s.split("//", 1)[0].strip()
        if not code or code.startswith(("*", "/*", "*/", "@", "package ", "import ")):
            if code.endswith(";") or code.endswith("{") or code.endswith("}"):
                buf = ""
                depth = 0
            brace += _code_braces(raw, cstate)
            i += 1
            continue
        if code in ("}", "};"):
            # lone block close: no statement can span it; drop any junk buffer
            buf = ""
            depth = 0
            brace += _code_braces(raw, cstate)
            i += 1
            continue
        if not buf:
            buf_start = line_starts[i]
            buf_brace = brace
        buf += " " + code
        depth += code.count("(") - code.count(")")
        is_end = (depth <= 0 and (code.endswith(";") or code.endswith("{"))) or len(buf) > 1200
        if buf and is_end:
            stmt = " ".join(buf.split())
            if buf_brace == 1 and "(" in stmt and ")" in stmt:
                mm = _sig_name_re.match(stmt)
                if mm:
                    _ret, name = mm.group(1) or "", mm.group(2)
                    if name not in JAVA_KEYWORDS and not stmt.startswith(
                            ("if ", "for ", "while ", "switch ", "catch ", "return ", "new ")):
                        key = (name, stmt[stmt.find("("):stmt.rfind(")") + 1])
                        if key not in seen:
                            seen.add(key)
                            sig = stmt[:300] + ("..." if len(stmt) > 300 else "")
                            kind = ("constructor" if name == main["name"] and not _ret.strip()
                                    else "method")
                            methods.append({"name": name, "kind": kind, "sig": sig,
                                            "doc": _clean_doc(pending_doc)})
            elif buf_brace == 1 and stmt.endswith(";") and " " in stmt.strip(";"):
                fm = _field_re.match(stmt)
                if fm and "(" not in stmt and ")" not in stmt:
                    for nm in [x.strip() for x in fm.group(2).split(",")]:
                        if nm and ("field", nm) not in seen:
                            seen.add(("field", nm))
                            fields.append({"name": nm, "sig": stmt[:200],
                                           "doc": _clean_doc(pending_doc)})
            buf = ""
            depth = 0
            pending_doc = ""
        brace += _code_braces(raw, cstate)
        i += 1
    main["methods"] = methods
    main["fields"] = fields
    return {"package": pkg, "path": path, "types": types}


def apidocs_url(pkg, name):
    return APIDOCS + "/" + pkg.replace(".", "/") + "/" + name + ".html"


class Index:
    def __init__(self):
        self.classes = []  # {name, package, kind, path}
        self.by_name = {}
        self.parsed = {}
        self.cache_path = os.path.join(
            os.path.dirname(os.path.abspath(__file__)), ".index_cache.json")
        self._disk_loaded = False

    def _sources_mtime(self):
        latest = 0.0
        for root, _dirs, files in os.walk(API_SRC):
            for fn in files:
                if fn.endswith(".java"):
                    try:
                        m = os.path.getmtime(os.path.join(root, fn))
                        if m > latest:
                            latest = m
                    except OSError:
                        pass
        return latest

    def _load_disk(self):
        try:
            with open(self.cache_path, encoding="utf-8") as f:
                data = json.load(f)
            if data.get("mtime", 0) >= self._sources_mtime():
                self.parsed = data.get("parsed", {})
                self._disk_loaded = True
                return True
        except (OSError, ValueError):
            pass
        return False

    def _save_disk(self):
        try:
            with open(self.cache_path, "w", encoding="utf-8") as f:
                json.dump({"mtime": self._sources_mtime(), "parsed": self.parsed}, f)
        except OSError:
            pass

    def ensure_parsed(self):
        if self.parsed or self._load_disk():
            return
        for c in self.classes:
            p = c["path"]
            if p not in self.parsed:
                try:
                    info = parse_java(p)
                except OSError:
                    continue
                # gameval ID files carry tens of thousands of constants; keep
                # a bounded sample on disk, totals are preserved separately
                for t in info["types"]:
                    t["field_total"] = len(t.get("fields", []))
                    t["fields"] = t.get("fields", [])[:300]
                self.parsed[p] = info
        self._save_disk()

    def build(self):
        for root, _dirs, files in os.walk(API_SRC):
            for fn in files:
                if not fn.endswith(".java"):
                    continue
                p = os.path.join(root, fn)
                info = scan_types(p)
                if info is None:
                    continue
                for t in info["types"]:
                    entry = {"name": t["name"], "package": info["package"],
                             "kind": t["kind"], "path": p}
                    self.classes.append(entry)
                    self.by_name.setdefault(t["name"], []).append(entry)

    def detail(self, name):
        cands = self.by_name.get(name, [])
        if not cands and "." in name:
            short = name.split(".")[-1]
            cands = self.by_name.get(short, [])
        if not cands:
            return None
        entry = cands[0]
        if entry["path"] not in self.parsed:
            self.parsed[entry["path"]] = parse_java(entry["path"])
        info = self.parsed[entry["path"]]
        main = info["types"][0] if info["types"] else {}
        pkg = info["package"]
        return {
            "name": entry["name"], "package": pkg, "kind": entry["kind"],
            "declaration": main.get("decl", ""),
            "doc": main.get("doc", ""),
            "total_methods": len(main.get("methods", [])),
            "total_fields": main.get("field_total", len(main.get("fields", []))),
            "methods": main.get("methods", []),
            "fields": main.get("fields", []),
            "path": entry["path"],
            "apidocs": APIDOCS + "/" + pkg.replace(".", "/") + "/" + entry["name"] + ".html",
            "aliases": len(cands) - 1,
        }


INDEX = Index()

TOOLS = [
    {"name": "api_list_classes",
     "description": "List RuneLite API classes/interfaces/enums (local Oct-2026 snapshot). Filter by name substring and/or kind.",
     "inputSchema": {"type": "object", "properties": {
         "filter": {"type": "string", "description": "Case-insensitive substring of class name"},
         "kind": {"type": "string", "enum": ["", "interface", "class", "enum"]},
         "limit": {"type": "integer", "default": 100}},
      "additionalProperties": False}},
    {"name": "api_class",
     "description": "Full detail for one API class: declaration, javadoc, methods with signatures+docs, fields/constants, local path, apidocs URL.",
     "inputSchema": {"type": "object", "properties": {
         "name": {"type": "string", "description": "Simple or qualified class name, e.g. Tile, SceneTileModel, DrawCallbacks"}},
      "required": ["name"], "additionalProperties": False}},
    {"name": "api_search",
     "description": "Substring search over runelite-api sources (method names, types, comments). Returns file:line snippets.",
     "inputSchema": {"type": "object", "properties": {
         "query": {"type": "string"},
         "scope": {"type": "string", "enum": ["methods", "all"], "default": "methods"},
         "limit": {"type": "integer", "default": 40}},
      "required": ["query"], "additionalProperties": False}},
    {"name": "shader_list",
     "description": "List staged reference shaders (reference-shaders/runelite-gpu).",
     "inputSchema": {"type": "object", "properties": {}, "additionalProperties": False}},
    {"name": "shader_read",
     "description": "Read a staged shader file, with optional line offset/limit.",
     "inputSchema": {"type": "object", "properties": {
         "path": {"type": "string", "description": "Relative path under reference-shaders/, e.g. runelite-gpu/vert.glsl"},
         "offset": {"type": "integer", "default": 1},
         "limit": {"type": "integer", "default": 200}},
      "required": ["path"], "additionalProperties": False}},
    {"name": "doc_list",
     "description": "List the RUNELITE_*.md guides in the workspace root.",
     "inputSchema": {"type": "object", "properties": {}, "additionalProperties": False}},
    {"name": "doc_read",
     "description": "Read a workspace guide (RUNELITE_*.md or reference-shaders/README.md), with optional line offset/limit.",
     "inputSchema": {"type": "object", "properties": {
         "name": {"type": "string", "description": "File name, e.g. RUNELITE_RUNTIME_RULES.md"},
         "offset": {"type": "integer", "default": 1},
         "limit": {"type": "integer", "default": 200}},
      "required": ["name"], "additionalProperties": False}},
    {"name": "source_list",
     "description": "List readable source roots: GPU plugin Java, deob construction files (Group E pins).",
     "inputSchema": {"type": "object", "properties": {}, "additionalProperties": False}},
    {"name": "source_read",
     "description": "Ranged read of one allowlisted Java source (GPU plugin or deob pin). line numbers are 1-based.",
     "inputSchema": {"type": "object", "properties": {
         "path": {"type": "string", "description": "gpu/<File>.java or deob/<File>.java, e.g. gpu/SceneUploader.java, deob/class150.java"},
         "offset": {"type": "integer", "default": 1},
         "limit": {"type": "integer", "default": 120}},
      "required": ["path"], "additionalProperties": False}},
]


def tool_api_list_classes(args):
    f = (args.get("filter") or "").lower()
    k = args.get("kind") or ""
    lim = min(int(args.get("limit") or 100), 500)
    out = [c for c in INDEX.classes
           if (not f or f in c["name"].lower()) and (not k or c["kind"] == k)]
    return {"total": len(out), "classes": [
        {"name": c["name"], "package": c["package"], "kind": c["kind"]} for c in out[:lim]]}


def tool_api_class(args):
    d = INDEX.detail(args.get("name", ""))
    if d is None:
        return {"error": "class not found: %s" % args.get("name", "")}
    d["methods"] = d["methods"][:200]
    d["fields"] = d["fields"][:200]
    return d


def tool_api_search(args):
    q = (args.get("query") or "").lower()
    scope = args.get("scope") or "methods"
    lim = min(int(args.get("limit") or 40), 100)
    hits = []
    for c in INDEX.classes:
        if q in c["name"].lower():
            hits.append({"match": "type:" + c["name"], "package": c["package"],
                         "kind": c["kind"], "path": c["path"]})
            if len(hits) >= lim:
                return {"query": args.get("query"), "hits": hits}
    if scope == "all":
        for root, _d, files in os.walk(API_SRC):
            for fn in files:
                if not fn.endswith(".java"):
                    continue
                p = os.path.join(root, fn)
                try:
                    with open(p, encoding="utf-8", errors="replace") as f:
                        for i, line in enumerate(f, 1):
                            if q in line.lower():
                                s = line.strip()
                                if len(s) > 220:
                                    s = s[:220] + "..."
                                hits.append({"match": s, "path": p, "line": i})
                                if len(hits) >= lim:
                                    return {"query": args.get("query"), "hits": hits}
                except OSError:
                    continue
    else:
        INDEX.ensure_parsed()
        for p, info in INDEX.parsed.items():
            if not info["types"]:
                continue
            pkg = info["package"]
            cname = info["types"][0]["name"]
            for m in info["types"][0].get("methods", []):
                if q in m["name"].lower():
                    hits.append({"match": m["sig"][:220], "doc": m["doc"][:160],
                                 "class": cname, "package": pkg, "path": p})
                    if len(hits) >= lim:
                        return {"query": args.get("query"), "hits": hits}
    return {"query": args.get("query"), "hits": hits}


def _safe_join(base, rel):
    p = os.path.normpath(os.path.join(base, rel))
    if not p.startswith(os.path.normpath(base) + os.sep) and p != os.path.normpath(base):
        return None
    return p


def tool_shader_list(_args):
    out = []
    for root, _d, files in os.walk(SHADERS):
        for fn in sorted(files):
            if fn.startswith("."):
                continue
            p = os.path.join(root, fn)
            out.append(os.path.relpath(p, os.path.join(WORKSPACE, "reference-shaders")))
    return {"files": out}


def tool_shader_read(args):
    p = _safe_join(os.path.join(WORKSPACE, "reference-shaders"), args.get("path", ""))
    if p is None or not os.path.isfile(p):
        return {"error": "not found: %s" % args.get("path", "")}
    off = max(int(args.get("offset") or 1), 1)
    lim = min(int(args.get("limit") or 200), 1000)
    with open(p, encoding="utf-8", errors="replace") as f:
        lines = f.readlines()
    total = len(lines)
    return {"path": args.get("path"), "total_lines": total,
            "lines": "".join(lines[off - 1: off - 1 + lim])}


def tool_doc_list(_args):
    out = ["index.md"]
    out += sorted(f for f in os.listdir(WORKSPACE)
                  if f.startswith("RUNELITE_") and f.endswith(".md"))
    out.append("reference-shaders/README.md")
    api_dir = os.path.join(WORKSPACE, "docs", "api")
    try:
        out += sorted("docs/api/" + f for f in os.listdir(api_dir) if f.endswith(".md"))
    except OSError:
        pass
    return {"docs": out}


def tool_doc_read(args):
    name = args.get("name", "")
    p = _safe_join(WORKSPACE, name)
    if p is None or not os.path.isfile(p):
        return {"error": "not found: %s" % name}
    off = max(int(args.get("offset") or 1), 1)
    lim = min(int(args.get("limit") or 200), 1000)
    with open(p, encoding="utf-8", errors="replace") as f:
        lines = f.readlines()
    return {"name": name, "total_lines": len(lines),
            "lines": "".join(lines[off - 1: off - 1 + lim])}


def tool_source_list(_args):
    gpu = sorted(f for f in os.listdir(GPU_JAVA) if f.endswith(".java"))
    return {"roots": {
        "gpu/": "runelite-client GPU plugin Java (upload, sort, zones, renderer)",
        "deob/": "deob construction pins (Group E: scene build, loc dispatch, terrain)"},
        "gpu": gpu, "deob": DEOB_FILES}


def tool_source_read(args):
    rel = (args.get("path") or "").replace("\\", "/")
    if rel.startswith("gpu/"):
        base, name = GPU_JAVA, rel[4:]
        ok = name.endswith(".java") and "/" not in name and os.path.isfile(os.path.join(base, name))
    elif rel.startswith("deob/"):
        base, name = _DEOB, rel[5:]
        ok = name in DEOB_FILES
    else:
        ok = False
    if not ok:
        return {"error": "not allowlisted: %s (see source_list)" % rel}
    p = os.path.join(base, name)
    off = max(int(args.get("offset") or 1), 1)
    lim = min(int(args.get("limit") or 120), 400)
    with open(p, encoding="utf-8", errors="replace") as f:
        lines = f.readlines()
    total = len(lines)
    numbered = ["%d: %s" % (n, ln) for n, ln in
                enumerate(lines[off - 1: off - 1 + lim], start=off)]
    return {"path": rel, "total_lines": total, "lines": numbered}


HANDLERS = {
    "api_list_classes": tool_api_list_classes,
    "api_class": tool_api_class,
    "api_search": tool_api_search,
    "shader_list": tool_shader_list,
    "shader_read": tool_shader_read,
    "doc_list": tool_doc_list,
    "doc_read": tool_doc_read,
    "source_list": tool_source_list,
    "source_read": tool_source_read,
}


def respond(mid, result):
    sys.stdout.write(json.dumps({"jsonrpc": "2.0", "id": mid, "result": result}) + "\n")
    sys.stdout.flush()


def fail(mid, code, message):
    if mid is None:
        return
    sys.stdout.write(json.dumps({"jsonrpc": "2.0", "id": mid,
                                 "error": {"code": code, "message": message}}) + "\n")
    sys.stdout.flush()


def log(msg):
    sys.stderr.write("[runelite-mcp] %s\n" % msg)
    sys.stderr.flush()


def main():
    INDEX.build()
    log("indexed %d types from %s" % (len(INDEX.classes), API_SRC))
    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        try:
            msg = json.loads(line)
        except ValueError:
            continue
        method = msg.get("method", "")
        mid = msg.get("id")
        params = msg.get("params") or {}
        try:
            if method == "initialize":
                respond(mid, {
                    "protocolVersion": PROTOCOL_VERSION,
                    "capabilities": {"tools": {}},
                    "serverInfo": {"name": "runelite-render-mcp", "version": "1.0.0"}})
            elif method == "tools/list":
                respond(mid, {"tools": TOOLS})
            elif method == "tools/call":
                name = (params.get("name") or "")
                handler = HANDLERS.get(name)
                if handler is None:
                    fail(mid, -32602, "unknown tool: %s" % name)
                else:
                    respond(mid, {"content": [{"type": "text", "text": json.dumps(
                        handler(params.get("arguments") or {}), indent=1)}]})
            elif method in ("notifications/initialized", "notifications/cancelled"):
                pass
            elif method == "ping":
                respond(mid, {})
            elif mid is not None:
                fail(mid, -32601, "method not found: %s" % method)
        except Exception as ex:  # never kill the session on a bad call
            log("error: %r" % ex)
            fail(mid, -32603, "internal error: %s" % ex)


if __name__ == "__main__":
    main()
