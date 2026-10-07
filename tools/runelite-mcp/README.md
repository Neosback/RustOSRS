# runelite-render-mcp

Dependency-free MCP server (Python 3 stdlib only — no `pip install` needed)
exposing the workspace's RuneLite render reference over MCP stdio:

- **API reference**: all 293 `runelite-api` sources from the local Oct-2026
  snapshot (`/Users/tylercovalt/Documents/runelite-master`), parsed to
  class/method/field level with javadoc summaries. No network at runtime;
  every class answer includes its `static.runelite.net` apidocs URL for humans.
- **Shaders**: `reference-shaders/runelite-gpu` listing + ranged reads.
- **Guides**: `RUNELITE_*.md` listing + ranged reads.

## Run

```bash
python3 server.py   # stdio transport; logs go to stderr
```

First methods-scope search builds `.index_cache.json` (~1 MB, ~12 s once);
warm calls answer in under a second. Delete the cache to force a rebuild
(e.g. after updating the local snapshot).

## Static API docs (`gen_api_docs.py`)

`python3 gen_api_docs.py` emits one Markdown file per render-relevant class
(49 classes, Groups C/D/F + `DrawCallbacks`) into `docs/api/` plus an index —
the same parser as the server, so MCP answers and checked-in docs never drift.
`--check` verifies the snapshot still contains every needed class (CI-friendly).
Re-run after updating the local snapshot.

## Tools

| Tool | What |
|---|---|
| `api_list_classes` | List types, optional name-substring + kind filter |
| `api_class` | Full detail: declaration, docs, methods, fields, apidocs URL |
| `api_search` | Substring search over method names (`scope=methods`) or full source text (`scope=all`), with `file:line` hits |
| `shader_list` / `shader_read` | Staged reference shaders |
| `doc_list` / `doc_read` | The workspace guides |
| `source_list` / `source_read` | GPU plugin Java + deob Group-E pins, ranged reads with line numbers (path-traversal safe allowlist) |

## Wire into a client
Claude Desktop (`claude_desktop_config.json`) or any MCP stdio client:

```json
{
  "mcpServers": {
    "runelite-render": {
      "command": "python3",
      "args": ["/Users/tylercovalt/Documents/ChatGPT/OpenRune-Map-Editor-Rust/tools/runelite-mcp/server.py"]
    }
  }
}
```

For opencode or other file-based configs, use the same command + args
with a `"type": "stdio"` transport entry.

## Notes / limits

- Corpus is the **local snapshot**, not live apidocs: fresher upstream renames
  (e.g. anything published after 4 Oct 2026) won't appear. Re-point `API_SRC`
  at the top of `server.py` to refresh.
- `gameval` ID classes (`ObjectID`, `ItemID`, …) carry tens of thousands of
  constants; responses cap fields at 200 with `total_fields` reported.
- Duplicate simple names across subpackages (e.g. two `ObjectID`s) resolve to
  the first match with an `aliases` count — pass a qualified name to disambiguate.
