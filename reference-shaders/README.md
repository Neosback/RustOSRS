# Reference Shaders — RuneLite GPU plugin (staged for WGSL porting)

Staged copies. Do not edit — port from these into `editor_render` WGSL.

## Provenance

- `vert.glsl`, `frag.glsl`, `vertui.glsl`, `fragui.glsl`, `hsl_to_rgb.glsl`,
  `colorblind.glsl`, `scale/*`, `regions/regions.txt`:
  `/Users/tylercovalt/Documents/runelite-master/runelite-client/src/main/resources/net/runelite/client/plugins/gpu/`
  (snapshot 4 Oct 2026 — newest; `vert`/`frag`/`hsl_to_rgb`/`ui`/`scale` are
  byte-identical to the Jan 2026 tree; `colorblind.glsl` here is the newer
  52-line version with the `colorblindIntensity` uniform; `regions.txt` here is
  the newer 465-line version with extra random-event regions).
- `comp.glsl`, `comp.cl`, `comp_unordered.glsl`, `comp_unordered.cl`,
  `priority_render.glsl`, `priority_render.cl`, `common.glsl`, `common.cl`,
  `cl_types.cl`:
  `/Users/tylercovalt/Documents/ChatGPT/RSPSi-resources/RuneLite-melxin/runelite-client/src/main/resources/net/runelite/client/plugins/gpu/`
  (Jan 2026 tree — the Oct 2026 snapshot does not ship these files on disk;
  neither tree's Java loads them by name, see `RUNELITE_RENDER_SOURCES.md`
  Group B; kept here as priority-math documentation for the CPU sorter).

RuneLite is BSD 2-clause licensed. These files remain property of their
respective authors; they are staged here as a porting reference only.

## Include graph (resolved by `template/Template.java` `#include`)

- `vert.glsl` → `#include texture_config` (generated: `TEXTURE_COUNT`) +
  `#include "hsl_to_rgb.glsl"`
- `frag.glsl` → `#include colorblind_mode` (generated) +
  `#include "hsl_to_rgb.glsl"` + `#include "colorblind.glsl"`
