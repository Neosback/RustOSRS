import PicoGL, { type App, type Texture } from "picogl";
import { HD_MATERIALS } from "./HdMaterialData";

export class HdMaterials {
    readonly lookup: Texture;
    readonly textures: Texture;
    private readonly lookupData = new Float32Array(1024 * 2 * 4);
    private readonly pixels = new Uint8Array(128 * 128 * 4 * (HD_MATERIALS.length + 1));
    private readonly loaded = new Set<number>();
    private started = false;
    private disposed = false;
    private dirty = true;
    private mapping = "";

    constructor(app: App) {
        this.lookup = app.createTexture2D(1024, 2, {
            internalFormat: PicoGL.RGBA32F, type: PicoGL.FLOAT,
            minFilter: PicoGL.NEAREST, magFilter: PicoGL.NEAREST,
        });
        this.textures = app.createTextureArray(new Uint8Array(4), 1, 1, 1, {
            minFilter: PicoGL.LINEAR, magFilter: PicoGL.LINEAR,
            wrapS: PicoGL.REPEAT, wrapT: PicoGL.REPEAT,
        });
    }

    update(layers: Map<number, number>): void {
        if (!this.started) {
            this.started = true;
            this.textures.resize(128, 128, HD_MATERIALS.length + 1);
            this.textures.data(this.pixels);
            HD_MATERIALS.forEach((material, index) => {
                if (!material.file) return;
                const image = new Image();
                image.onload = () => {
                    if (this.disposed) return;
                    const canvas = document.createElement("canvas");
                    canvas.width = canvas.height = 128;
                    const context = canvas.getContext("2d");
                    if (!context) return;
                    try {
                        context.drawImage(image, 0, 0, 128, 128);
                        this.pixels.set(context.getImageData(0, 0, 128, 128).data, (index + 1) * 128 * 128 * 4);
                        this.loaded.add(index);
                        this.dirty = true;
                    } catch (error) { console.warn("117 HD: texture unavailable", error); }
                };
                image.onerror = () => { console.warn("117 HD: texture unavailable", material.file); };
                image.src = material.file;
            });
        }
        const mapping = HD_MATERIALS.map(m => layers.get(m.id) ?? 0).join(",");
        if (!this.dirty && this.mapping === mapping) return;
        this.lookupData.fill(0);
        HD_MATERIALS.forEach((material, index) => {
            const layer = layers.get(material.id);
            if (layer === undefined || layer >= 1024) return;
            this.lookupData.set(material.params, layer * 4);
            this.lookupData.set([this.loaded.has(index) ? index + 1 : 0, material.unlit ? 1 : 0, 0, 0], (1024 + layer) * 4);
        });
        this.lookup.data(this.lookupData);
        this.textures.data(this.pixels);
        this.mapping = mapping;
        this.dirty = false;
    }

    dispose(): void {
        this.disposed = true;
        this.lookup.delete();
        this.textures.delete();
    }
}
