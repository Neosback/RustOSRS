# 117 HD

Opt in through **xRSPS → 117 HD**. Disabled on fresh installs; explicit choices
persist in browser storage. No renderer restart is needed.

The plugin adapts the sibling `elvarg-web-client` HD renderer's non-water lighting,
regional environments, fog, material parameters, texture replacements, animated
object lights and directional shadow mapping to this client's PicoGL renderer.
Shaders, data, textures and GPU-resource ownership stay in this directory. Core
changes register the plugin and expose shader, frame, draw and disposal hooks.

The existing water shader, texture assets, seabed and shoreline generation are
unchanged and do not depend on this toggle. HD replacements use a separate texture
array, leaving the original textures available immediately on disable.

Compatibility limits: packed client meshes have no normal stream, so lighting
uses geometric normals rather than the source's smooth model/terrain normals.
Source terrain recoloring/ground-material recipes are not applied to this client's
terrain pipeline. Point lights are limited to the nearest 8 on the active plane.
Shadow maps update at 30 Hz, with immediate refreshes on teleports, plane changes,
environment changes and roof/draw-distance changes.

Run `yarn test:hd` in `client`. To generate the optional WebGL browser check:
`yarn test:hd /tmp/elvarg-hd-shader-check.html`.

117HD-derived data/assets use the license at
`client/public/images/water/LICENSE-117HD.txt`.
