"""Source-level editor ownership ratchet and built-in feature navigation contracts.

This is a deliberately small Rust lexical reader, not a semantic Rust parser. The compiler
owns visibility enforcement; this guard prevents unreviewed growth of the application surface.
"""
from __future__ import annotations

from collections import Counter
import json
import re
from pathlib import Path


_START = re.compile(r'//|/\*|(?:br|r)(?P<hashes>\#*)"|b?"|b?\'(?:\\(?:u\{[^}]*\}|x[0-9a-fA-F]{2}|.)|[^\'\\\n])\'')


def rust_code(text: str) -> str:
    """Mask comments and literals while preserving character positions and line breaks."""
    chunks: list[str] = []
    cursor = 0
    while match := _START.search(text, cursor):
        chunks.append(text[cursor:match.start()])
        start = match.start()
        token = match.group()
        if token == "//":
            end = text.find("\n", match.end())
            end = len(text) if end < 0 else end
        elif token == "/*":
            depth, end = 1, match.end()
            while depth:
                nested = re.search(r"/\*|\*/", text[end:])
                if nested is None:
                    end = len(text)
                    break
                depth += 1 if nested.group() == "/*" else -1
                end += nested.end()
        elif match.group("hashes") is not None:
            terminator = '"' + match.group("hashes")
            end = text.find(terminator, match.end())
            end = len(text) if end < 0 else end + len(terminator)
        elif token.endswith("'"):
            end = match.end()
        else:
            end = match.end()
            while end < len(text):
                if text[end] == "\\":
                    end += 2
                elif text[end] == '"':
                    end += 1
                    break
                else:
                    end += 1
        chunks.append(re.sub(r"[^\n]", " ", text[start:end]))
        cursor = end
    chunks.append(text[cursor:])
    return "".join(chunks)


def block_end(code: str, opening: int) -> int:
    depth = 1
    for position in range(opening + 1, len(code)):
        depth += (code[position] == "{") - (code[position] == "}")
        if depth == 0:
            return position
    raise ValueError("unclosed Rust source block")


def app_methods(text: str) -> list[str]:
    code = rust_code(text)
    methods: list[str] = []
    for match in re.finditer(r"\bimpl\b[^{};]*\bEditorApp\s*\{", code):
        end = block_end(code, match.end() - 1)
        body = code[match.end():end]
        depth = 0
        for token in re.finditer(r"[{}]|\bfn\s+([A-Za-z_][A-Za-z0-9_]*)", body):
            if token.group() == "{":
                depth += 1
            elif token.group() == "}":
                depth -= 1
            elif depth == 0:
                methods.append(token.group(1))
    return sorted(methods)


def app_surface(editor_source: Path) -> dict[str, list[str]]:
    result = {}
    for path in sorted(editor_source.rglob("*.rs")):
        if methods := app_methods(path.read_text(encoding="utf-8")):
            result[path.relative_to(editor_source).as_posix()] = methods
    return result


def app_fields(editor_source: Path) -> list[str]:
    code = rust_code((editor_source / "app.rs").read_text(encoding="utf-8"))
    match = re.search(r"\bstruct\s+EditorApp\s*\{", code)
    if match is None:
        raise ValueError("missing EditorApp composition root")
    body = code[match.end():block_end(code, match.end() - 1)]
    return sorted(re.findall(r"^\s*(\w+)\s*:", body, flags=re.M))


def check_app_surface(editor_source: Path, baseline: dict) -> list[str]:
    errors = []
    current = app_surface(editor_source)
    expected = baseline.get("methods", {})
    for path in sorted(set(current) | set(expected)):
        added = Counter(current.get(path, [])) - Counter(expected.get(path, []))
        removed = Counter(expected.get(path, [])) - Counter(current.get(path, []))
        if added:
            errors.append(f"EditorApp/{path}: new broad app methods {sorted(added.elements())}; put feature behavior on its owner")
        if removed:
            errors.append(f"EditorApp/{path}: obsolete method baseline {sorted(removed.elements())}; lower the ownership ratchet")
    fields = app_fields(editor_source)
    if fields != baseline.get("fields"):
        errors.append("EditorApp: composition fields changed; review canonical ownership and update the explicit surface baseline")
    return errors


def check_features(repo: Path, registry: dict) -> list[str]:
    errors = []
    code = rust_code((repo / "crates/osrs-editor/src/paint.rs").read_text(encoding="utf-8"))
    match = re.search(r"\benum\s+Tool\s*\{", code)
    if match is None:
        return ["feature map: missing Tool enum"]
    body = code[match.end():block_end(code, match.end() - 1)]
    tools = set(re.findall(r"^\s*(\w+)\s*,", body, flags=re.M))
    registered_tools = {name for name, feature in registry.items() if feature.get("kind", "tool") == "tool"}
    if tools != registered_tools:
        errors.append(f"feature map: Tool variants and feature entries differ: {sorted(tools ^ registered_tools)}")
    for tool, feature in registry.items():
        kind = feature.get("kind", "tool")
        if kind not in {"tool", "workspace"} or kind == "workspace" and tool in tools:
            errors.append(f"feature map/{tool}: invalid feature kind")
        required = ["controller"]
        if feature.get("mutates_document"):
            required += ["planner", "gateway", "planning_test", "cancellation_test", "commit_test", "undo_test"]
        for role in required:
            reference = feature.get(role)
            if not isinstance(reference, dict):
                errors.append(f"feature map/{tool}: missing {role} reference")
                continue
            file, symbol = reference.get("file"), reference.get("symbol")
            if not isinstance(file, str) or Path(file).is_absolute() or ".." in Path(file).parts:
                errors.append(f"feature map/{tool}/{role}: invalid repository-relative path")
                continue
            path = repo / file
            if not path.is_file() or not isinstance(symbol, str) or not re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", symbol):
                errors.append(f"feature map/{tool}/{role}: missing file or invalid symbol")
                continue
            source = rust_code(path.read_text(encoding="utf-8"))
            function = re.search(r"\bfn\s+" + re.escape(symbol) + r"\b", source)
            if function is None:
                errors.append(f"feature map/{tool}/{role}: {file} has no function {symbol}")
            elif role.endswith("_test"):
                prefix = source[:function.start()]
                if not re.search(r"#\s*\[\s*test\s*\]\s*$", prefix):
                    errors.append(f"feature map/{tool}/{role}: {symbol} is not a #[test] function")
    return errors


def check_editor_contracts(repo: Path) -> list[str]:
    try:
        baseline = json.loads((repo / "scripts/editor_app_surface.json").read_text(encoding="utf-8"))
        features = json.loads((repo / "scripts/editor_features.json").read_text(encoding="utf-8"))
        if baseline.get("version") != 1 or not isinstance(baseline.get("methods"), dict) or not isinstance(features, dict):
            return ["editor contracts: invalid surface baseline or feature map"]
        return check_app_surface(repo / "crates/osrs-editor/src", baseline) + check_features(repo, features)
    except (OSError, ValueError, TypeError) as error:
        return [f"editor contracts: {error}"]
