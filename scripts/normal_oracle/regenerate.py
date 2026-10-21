#!/usr/bin/env python3
"""Execute pinned ModelData normal methods; compare or write external candidates only."""

import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
ACCEPTED = ROOT / "reference-fixtures"


def extract_method(source, signature):
    """Extract a unique pinned declaration, preserving its method body byte for byte."""
    if source.count(signature) != 1:
        raise ValueError(f"Expected exactly one method: {signature}")
    start = source.index(signature)
    end = source.index("{", start) + 1
    depth = 1
    while depth and end < len(source):
        depth += (source[end] == "{") - (source[end] == "}")
        end += 1
    if depth:
        raise ValueError(f"Unclosed method: {signature}")
    return source[start:end]


def load_sources(source_root):
    pins = json.loads((HERE / "sources.json").read_text())
    result = {}
    for pin in pins["sources"]:
        path = source_root / pin["path"]
        data = path.read_bytes()
        if hashlib.sha256(data).hexdigest() != pin["sha256"]:
            raise ValueError(f"Source pin mismatch: {path}")
        result[path.name] = data.decode("utf-8")
    return result


def candidate_directory(path):
    path = path.resolve()
    if path.is_relative_to(ACCEPTED.resolve()):
        raise ValueError("Candidate output may not be written inside reference-fixtures")
    if path.exists():
        raise ValueError("Candidate output directory already exists; refusing overwrite")
    return path


def encode_model(model):
    vertices, faces = model["vertices"], model["faces"]
    if len(vertices) > 10000:
        raise ValueError("Model exceeds pinned merge marker capacity")
    values = [len(vertices), len(faces)]
    for vertex in vertices:
        values.extend(vertex[key] for key in ("x", "y", "z"))
    for face in faces:
        values.extend(face[key] for key in ("a", "b", "c"))
    types = model["face_render_types"]
    if types is not None and len(types) != len(faces):
        raise ValueError("Face render type count differs from face count")
    values.append(int(types is not None))
    values.extend(types or [])
    return values


def encode_input(document):
    if document["schema_version"] != 1:
        raise ValueError("Unsupported normalized schema version")
    case = document["case"]
    kind = case["kind"]
    if kind == "base_normals":
        values = [0, *encode_model(case["model"])]
    elif kind == "normal_merge":
        values = [1, *encode_model(case["left"]), *encode_model(case["right"])]
        values.extend(case["translation"][key] for key in ("x", "y", "z"))
        values.append(int(case["hide_matched_faces"]))
    else:
        raise ValueError(f"Unsupported normal oracle kind: {kind}")
    return " ".join(map(str, values)).encode()


def generate(source_root, fixtures):
    sources = load_sources(source_root)
    model = sources["ModelData.java"]
    methods = "\n".join(extract_method(model, signature) for signature in (
        "public void calculateVertexNormals()",
        "void calculateBounds()",
        "static void method5262(",
    ))
    constructor = extract_method(sources["VertexNormal.java"], "VertexNormal(VertexNormal var1)")
    # Only field storage and the unused Renderable height superclass are substituted.
    # All integer geometry, bounds, matching, accumulation and suppression bodies are original.
    shell = (HERE / "model_fields.java.txt").read_text()
    with tempfile.TemporaryDirectory(prefix="rustosrs-normal-oracle-") as temporary:
        work = Path(temporary)
        (work / "ModelData.java").write_text(shell + methods + "\n}\n")
        (work / "VertexNormal.java").write_text(
            "class VertexNormal { int x, y, z, magnitude; VertexNormal() {}\n"
            + constructor + "\n}\n"
        )
        (work / "FaceNormal.java").write_text("class FaceNormal { int x, y, z; }\n")
        subprocess.run([
            "javac", "--release", "17", "-d", str(work),
            str(work / "ModelData.java"), str(work / "VertexNormal.java"),
            str(work / "FaceNormal.java"), str(HERE / "NormalOracle.java"),
        ], check=True, timeout=60)
        results = {}
        for path in sorted(fixtures.glob("*.input.json")):
            encoded = encode_input(json.loads(path.read_text()))
            output = subprocess.check_output(
                ["java", "-cp", str(work), "NormalOracle"], input=encoded, timeout=30,
            )
            results[path.name.replace(".input.json", ".expected.json")] = json.loads(output)
        if not results:
            raise ValueError("No normal inputs found")
        return results


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--runelite-root", required=True, type=Path)
    parser.add_argument("--fixtures", type=Path, default=ACCEPTED / "normals")
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--check", action="store_true")
    mode.add_argument("--output", type=Path, help="New candidate directory outside accepted fixtures")
    args = parser.parse_args()
    candidate = candidate_directory(args.output) if args.output else None
    results = generate(args.runelite_root, args.fixtures)
    if args.check:
        for name, actual in results.items():
            if actual != json.loads((args.fixtures / name).read_text()):
                raise ValueError(f"Pinned normal oracle mismatch: {name}")
            print(f"MATCH {name}")
    else:
        candidate.mkdir(parents=True, exist_ok=False)
        for name, actual in results.items():
            (candidate / name).write_text(json.dumps(actual, indent=2) + "\n")
        print(f"Wrote {len(results)} candidates to {candidate}")


if __name__ == "__main__":
    main()
