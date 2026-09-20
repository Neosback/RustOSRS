import assert from "node:assert/strict";

import { resolveCacheDecodeProfile } from "../rs/cache/CacheDecodeProfile";
import { parseScriptFromBytes } from "../rs/cs2/Script";

/** Encodes both observed OSRS client-script trailer layouts. */
function encodeOsrsScript(longCounts: boolean): Int8Array {
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

    for (const c of "test_script") u8(c.charCodeAt(0));
    u8(0); // null-terminated name

    // 3 opcodes: push_int(7), sconst("hi"), return(0)
    u16(0);
    u32(7);
    u16(3);
    for (const c of "hi") u8(c.charCodeAt(0));
    u8(0);
    u16(21);
    u8(0);

    u32(3); // numOpcodes
    u16(5); // localIntCount
    u16(2); // localObjCount
    if (longCounts) u16(3); // localLongCount
    u16(1); // intArgCount
    u16(1); // objArgCount
    if (longCounts) u16(2); // longArgCount
    u8(0); // switch block count
    u16(1); // switch block length, counting the count byte (trailing 2 bytes)

    return new Int8Array(bytes.map((b) => (b << 24) >> 24));
}

function parsesOsrsTrailer(): void {
    const profile236 = resolveCacheDecodeProfile({ game: "oldschool", revision: 236 });
    const profile237 = resolveCacheDecodeProfile({ game: "oldschool", revision: 237 });
    const script236 = parseScriptFromBytes(42, encodeOsrsScript(false), profile236);
    const script237 = parseScriptFromBytes(43, encodeOsrsScript(true), profile237);

    for (const script of [script236, script237]) {
        assert.equal(script.name, "test_script");
        assert.equal(script.instructions.length, 3);
        assert.equal(script.localIntCount, 5);
        assert.equal(script.localObjCount, 2);
        assert.equal(script.intArgCount, 1);
        assert.equal(script.objArgCount, 1);
        assert.deepEqual(Array.from(script.instructions), [0, 3, 21]);
        assert.equal(script.intOperands[0], 7);
        assert.equal(script.stringOperands[1], "hi");
    }
    assert.equal(script236.localLongCount, 0);
    assert.equal(script236.longArgCount, 0);
    assert.equal(script237.localLongCount, 3);
    assert.equal(script237.longArgCount, 2);
}

parsesOsrsTrailer();
console.log("CS2 script parse tests passed");
