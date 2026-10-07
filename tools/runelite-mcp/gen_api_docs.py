#!/usr/bin/env python3
"""Generate per-class API reference MDs for the render-relevant subset.

Reads the local runelite-master sources (via server.py's parser) and emits
one Markdown file per needed class into <workspace>/docs/api/, plus an index.

Run:  python3 gen_api_docs.py [--check]
  --check: verify output is current without writing (CI-friendly).

Stdlib only.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from server import Index, APIDOCS  # noqa: E402

WORKSPACE = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
OUT_DIR = os.path.join(WORKSPACE, "docs", "api")

# (class name, guide section, why we need it)
NEEDED = [
    ("Scene", "C1", "Scene root: extended tiles/settings/heights/shapes/roofs/WorldView."),
    ("Tile", "C2", "Per-tile slots + bridge/render-level contract."),
    ("SceneTilePaint", "C3", "Flat underlay quads (corner HSL, texture, flat flag)."),
    ("SceneTileModel", "C4", "Shaped overlay cuts (shape/rotation/faces/colors)."),
    ("WallObject", "C5", "Dual-slot walls + orientation flags."),
    ("DecorativeObject", "C6", "Wall decor + standoff offsets."),
    ("GroundObject", "C7", "Floor decals at sampled height."),
    ("GameObject", "C8", "Multi-tile objects, footprints, config word."),
    ("TileObject", "C9", "Base id/position/plane contract."),
    ("Renderable", "C9", "Drawable marker inherited by all locs."),
    ("DynamicObject", "C9", "Animated locs (model swap per frame)."),
    ("ItemLayer", "C9", "Ground-item stacks (must not crash the uploader)."),
    ("EntityOps", "C9", "Menu-op contract from ObjectComposition.getOps."),
    ("WorldView", "C10", "TOPLEVEL/instance coords, tile-height helper."),
    ("Perspective", "D1", "Tile math, trig tables, canvas projection."),
    ("Constants", "D2", "Sizes, tile flags, shifts."),
    ("JagexColor", "D3", "HSL pack/unpack + gamma conversion."),
    ("Model", "D4", "Renderable mesh: faces, priorities, bias, textures, alpha."),
    ("ModelData", "D4", "Mutable mesh: clones, normals, recolor, resize, toModel."),
    ("Mesh", "D4", "Shared mesh base (float verts, indices, textures)."),
    ("Texture", "D5", "Animation direction + speed."),
    ("TextureProvider", "D5", "Texture array, brightness, default colors."),
    ("ObjectComposition", "D6", "Sizes, multiloc morph chain, ops."),
    ("Projection", "D7", "project() contract used by the sorter cull."),
    ("IntProjection", "D7", "Integer projection variant."),
    ("FloatProjection", "D7", "Float projection variant."),
    ("Client", "D7", "Accessor shapes only (heights, camera, provider, views)."),
    ("LocalPoint", "D7", "Scene-unit coordinates."),
    ("WorldPoint", "D7", "Global tile coordinates."),
    ("Angle", "D7", "JAU angle convention."),
    ("Direction", "D7", "JAU cardinal bands (mirroring arbiter)."),
    ("WorldArea", "D7", "Footprint rectangles."),
    ("DrawCallbacks", "A10", "GPU flags, passes, per-zone/terrain/frustum hooks."),
    ("Jarvis", "F1", "Convex-hull clickboxes for picking."),
    ("SimplePolygon", "F1", "Hull/selection polygon container."),
    ("Point", "F1", "2D point used by footprints, hulls, picking."),
    ("AABB", "F1", "Bounding boxes for frustum/bounds checks."),
    ("CollisionData", "F2", "Per-tile collision words for the validity overlay."),
    ("CollisionDataFlag", "F2", "Collision flag constants."),
    ("RuneLiteObject", "F3", "Custom scene objects: placement ghosts/markers."),
    ("RuneLiteObjectController", "F3", "Controller side of custom scene objects."),
    ("Animation", "F4", "Animation state (id, frames, duration) for previews."),
    ("AnimationController", "F4", "tick()/animate(Model) skeletal posing."),
    ("GraphicsObject", "F4", "Spotanim FX state (id, location, cycle)."),
    ("ItemComposition", "F5", "Item models for ground-item rendering."),
    ("TileItem", "F5", "Single ground item on an ItemLayer."),
    ("SpritePixels", "F5", "Pixel container behind texture upload."),
    ("VarbitComposition", "F6", "Varbit bit-range for morph extraction."),
    ("InstanceTemplates", "F6", "Instanced chunk templates + matcher."),
]

SKIP_METHOD_DOCS_FOR = {"ObjectID", "ItemID"}  # not in NEEDED; guard anyway


def md_for(name, section, why, detail):
    L = []
    L.append("# %s" % name)
    L.append("")
    L.append("`%s` — %s render contract (§%s). %s" % (
        detail["package"] + "." + name, detail["kind"], section, why))
    L.append("")
    L.append("Source: `%s`" % detail["path"])
    L.append("Apidocs: %s" % detail["apidocs"])
    L.append("")
    if detail["doc"]:
        L.append("> %s" % detail["doc"])
        L.append("")
    L.append("Declaration: `%s`" % detail["declaration"])
    L.append("")
    methods = detail["methods"]
    L.append("## Methods (%d)" % detail["total_methods"])
    L.append("")
    if name == "Client":
        L.append("_Large interface — render-relevant accessors called out; "
                 "see MCP `api_class(Client)` for all %d methods._" % detail["total_methods"])
        L.append("")
        keep = ("TileHeight", "Camera", "TextureProvider", "WorldView", "Skybox",
                "DrawDistance", "ExpandedMapLoading", "GameState", "GameCycle",
                "Plane", "Canvas")
        methods = [m for m in methods if any(k in m["name"] for k in keep)]
        L.append("### Render-relevant subset (%d shown)" % len(methods))
        L.append("")
    for m in methods:
        L.append("### `%s`" % m["sig"])
        if m["doc"]:
            L.append("%s" % m["doc"])
        L.append("")
    fields = detail["fields"]
    if fields or detail["total_fields"]:
        L.append("## Fields (%d%s)" % (detail["total_fields"],
                                       ", showing %d" % len(fields) if detail["total_fields"] > len(fields) else ""))
        L.append("")
        for f in fields:
            line = "- `%s`" % f["sig"]
            if f["doc"]:
                line += " — %s" % f["doc"]
            L.append(line)
        L.append("")
    L.append("_Generated from the local Oct-2026 snapshot by "
             "`tools/runelite-mcp/gen_api_docs.py`. Regenerate after updating the snapshot._")
    L.append("")
    return "\n".join(L)


def build_index():
    idx = Index()
    idx.build()
    return idx


def main():
    check = "--check" in sys.argv
    idx = build_index()
    missing = [n for n, _s, _w in NEEDED
               if not idx.by_name.get(n) and not idx.by_name.get(n.split(".")[-1])]
    if missing:
        print("MISSING from snapshot: %s" % ", ".join(missing))
        return 1
    if not check:
        os.makedirs(OUT_DIR, exist_ok=True)
    index_lines = ["# API Reference (render subset)", "",
                   "%d classes needed for full map-scene rendering, generated "
                   "from the local snapshot. See `RUNELITE_RENDER_SOURCES.md` "
                   "Groups C/D (§-refs) for why each exists." % len(NEEDED), ""]
    dirty = False
    for name, section, why in NEEDED:
        detail = idx.detail(name)
        detail["methods"] = detail["methods"][:400]
        text = md_for(name, section, why, detail)
        index_lines.append("- [%s](%s.md) — §%s — %s" % (name, name, section, why))
        if check:
            continue
        path = os.path.join(OUT_DIR, name + ".md")
        if not os.path.isfile(path) or open(path, encoding="utf-8").read() != text:
            with open(path, "w", encoding="utf-8") as f:
                f.write(text)
            dirty = True
    if not check:
        index_text = "\n".join(index_lines) + "\n"
        ipath = os.path.join(OUT_DIR, "README.md")
        if not os.path.isfile(ipath) or open(ipath, encoding="utf-8").read() != index_text:
            with open(ipath, "w", encoding="utf-8") as f:
                f.write(index_text)
            dirty = True
        print("wrote %d files to %s%s" % (len(NEEDED) + 1, OUT_DIR, "" if dirty else " (unchanged)"))
    else:
        print("all %d classes present in snapshot" % len(NEEDED))
    return 0


if __name__ == "__main__":
    sys.exit(main())
