# Third-party material

## 117 HD (RLHD) - `assets/117hd/`

Shaders (`*.glsl`), scene data (`scene/*.json`), textures (`scene/textures`), JSON schemas and the
settings guide are copied verbatim from the 117 HD RuneLite plugin
(<https://github.com/117HD/RLHD>, version 1.5.2 snapshot taken 2026-10-11 from `~/Documents/RLHD-master`).
Do not hand-edit these files; refresh them from upstream so diffs stay reviewable. The Rust port of the
plugin's logic lives in `crates/osrs-hd` and keeps the upstream notices.

### Ported code (BSD-2, same notice as below)

Logic ported or adapted from the plugin's Java and GLSL sources keeps that notice and names its origin in
the file header:

| File | Origin |
|---|---|
| `crates/osrs-hd/src/lib.rs` | data model of the plugin's `scene/*.json` files (header links the upstream) |
| `crates/osrs-render/src/gpu/hd/layout.rs`, `hd/materials.wgsl` | `opengl/uniforms/UBOMaterials.java`, `Material.fillMaterialStruct` (material table layout and packing) |
| `crates/osrs-render/src/gpu/hd/materials.rs` | `MaterialManager.swapMaterials` (texture layer assignment), `TextureManager.uploadTexture` (flip, bicubic resample to 128/256 px, sRGB mip chain) |

`gpu/hd/post.wgsl` and the frame graph are original; the placeholder resolve pass is replaced by ports of
`utils/tone_mapping.glsl` (MIT, (c) 2022 Bjorn Ottosson, see the header of that file), `color_filters.glsl`
and `color_blindness.glsl` in phase 8, which will carry their notices.

BSD 2-Clause License

Copyright (c) 2018, Adam <Adam@sigterm.info>
Copyright (c) 2021, 117 <https://twitter.com/117scape>
Copyright (c) 2021, Hooder <ahooder@protonmail.com> and contributors (see the file headers)
All rights reserved.

Redistribution and use in source and binary forms, with or without modification, are permitted
provided that the following conditions are met:

1. Redistributions of source code must retain the above copyright notice, this list of conditions and
   the following disclaimer.
2. Redistributions in binary form must reproduce the above copyright notice, this list of conditions
   and the following disclaimer in the documentation and/or other materials provided with the
   distribution.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS" AND ANY EXPRESS OR IMPLIED
WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A
PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT OWNER OR CONTRIBUTORS BE LIABLE FOR
ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT
LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS
INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR
TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF
ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
