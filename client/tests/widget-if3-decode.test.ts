import assert from "node:assert/strict";

import type { CacheInfo } from "../rs/cache/CacheInfo";
import { CacheSystem } from "../rs/cache/CacheSystem";
import { WidgetLoader } from "../widgets/WidgetLoader";

/** Encodes both observed IF3 type-6 model-id widths. */
function encodeModelWidget(modelIdBytes: 2 | 4): Int8Array {
    const bytes: number[] = [];
    const u8 = (v: number) => bytes.push(v & 0xff);
    const u16 = (v: number) => {
        u8(v >> 8);
        u8(v);
    };
    const u32 = (v: number) => {
        u16(v >> 16);
        u16(v);
    };
    const str = (s: string) => {
        for (const c of s) u8(c.charCodeAt(0));
        u8(0);
    };

    u8(0xff); // IF3 marker
    u8(6); // type: model
    u16(0); // contentType
    u16(10); // x
    u16(20); // y
    u16(30); // width
    u16(40); // height
    u8(0); // widthMode
    u8(0); // heightMode
    u8(0); // xPositionMode
    u8(0); // yPositionMode
    u16(0xffff); // parentId: none
    u8(0); // hidden

    if (modelIdBytes === 2) u16(1234);
    else u32(1234);
    u16(0); // offsetX2d
    u16(0); // offsetY2d
    u16(0); // rotationX
    u16(0); // rotationY
    u16(0); // rotationZ
    u16(100); // zoom
    u16(0xffff); // animation: none
    u8(0); // orthogonal
    u16(0); // unused

    u8(0); // flags (3 bytes)
    u8(0);
    u8(0);
    str(""); // dataText
    u8(0); // action count
    u8(0); // dragZoneSize
    u8(0); // dragThreshold
    u8(0); // isScrollBar
    str(""); // spellActionName
    for (let i = 0; i < 18; i++) u8(0); // empty listeners
    for (let i = 0; i < 3; i++) u8(0); // empty trigger arrays

    return new Int8Array(bytes.map((b) => (b << 24) >> 24));
}

function cacheInfo(revision: number): CacheInfo {
    return {
        name: `test-${revision}`,
        game: "oldschool",
        environment: "live",
        revision,
        timestamp: "",
        size: 0,
    };
}

function decodeModelWidget(revision: number, modelIdBytes: 2 | 4): any {
    const data = encodeModelWidget(modelIdBytes);
    const index = {
        getFileIds: () => new Int32Array([0]),
        getFile: () => ({ data }),
    };
    const cache = new CacheSystem(
        [undefined, undefined, undefined, index as never],
        undefined,
        undefined,
        cacheInfo(revision),
    );
    return new WidgetLoader(cache).loadWidgetGroup(900)?.widgets.get((900 << 16) | 0);
}

function decodesModelWidget(): void {
    const widgets = [decodeModelWidget(236, 2), decodeModelWidget(237, 4)];

    for (const widget of widgets) {
        assert.ok(widget, "type-6 widget failed to decode");
        assert.equal(widget.type, 6);
        assert.equal(widget.modelId, 1234);
        assert.equal(widget.modelZoom, 100);
        assert.equal(widget.sequenceId, -1);
        assert.equal(widget.rawWidth, 30);
        assert.equal(widget.rawHeight, 40);
    }
}

decodesModelWidget();
console.log("IF3 widget decode tests passed");
