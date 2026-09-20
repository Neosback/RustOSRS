/**
 * `::runes` must reach the server. Cache script 7304 (the ::emote shortcut)
 * matches its keywords by prefix, so it would claim "runes" for the Run emote
 * and the chat script would then discard the command; the VM only lets that
 * script run for an exact emote name.
 */
import assert from "node:assert/strict";

import { resolveCacheDecodeProfile } from "../rs/cache/CacheDecodeProfile";
import { Cs2Vm } from "../rs/cs2/Cs2Vm";
import { EMOTE_SHORTCUT_SCRIPT_ID, isEmoteShortcut } from "../rs/cs2/EmoteShortcuts";
import { Opcodes } from "../rs/cs2/Opcodes";
import { parseScriptFromBytes, type Script } from "../rs/cs2/Script";

const profile = resolveCacheDecodeProfile({ game: "oldschool", revision: 237 });

type Instruction = { opcode: number; int?: number; string?: string };

/** Assembles a script the same way the cache stores one. */
function assemble(
    id: number,
    name: string,
    body: Instruction[],
    args: { int: number; obj: number },
): Script {
    const bytes: number[] = [];
    const u8 = (v: number) => bytes.push(v & 0xff);
    const u16 = (v: number) => (u8(v >> 8), u8(v));
    const u32 = (v: number) => (u16(v >> 16), u16(v));

    for (const char of name) u8(char.charCodeAt(0));
    u8(0);

    for (const { opcode, int = 0, string } of body) {
        u16(opcode);
        if (opcode === Opcodes.SCONST) {
            for (const char of string ?? "") u8(char.charCodeAt(0));
            u8(0);
        } else if (opcode === Opcodes.RETURN) {
            u8(int);
        } else if (opcode < 100) {
            u32(int);
        } else {
            u8(int);
        }
    }

    u32(body.length); // numOpcodes
    u16(0); // localIntCount
    u16(1); // localObjCount
    u16(0); // localLongCount
    u16(args.int); // intArgCount
    u16(args.obj); // objArgCount
    u16(0); // longArgCount
    u8(0); // switch block count
    u16(1); // switch block length, counting the count byte

    return parseScriptFromBytes(id, new Int8Array(bytes.map((b) => (b << 24) >> 24)), profile);
}

// Stands in for cache script 7304: takes the typed line and reports "emote
// handled", so a 1 back means the shortcut script ran and ate the command.
const emoteScript = assemble(
    EMOTE_SHORTCUT_SCRIPT_ID,
    "emote_shortcut",
    [
        { opcode: Opcodes.ICONST, int: 1 },
        { opcode: Opcodes.RETURN, int: 0 },
    ],
    { int: 0, obj: 1 },
);

const scripts = new Map<number, Script>([[EMOTE_SHORTCUT_SCRIPT_ID, emoteScript]]);
const vm = new Cs2Vm({
    loadScript: (id: number) => scripts.get(id),
    widgetManager: { beginBatch: () => {}, endBatch: () => {}, flushBatch: () => {} },
} as any);

/** Runs `emote_shortcut(typed)` the way the chat script does, returns its result. */
function askEmoteScript(typed: string): number {
    const caller = assemble(
        1,
        "chat_send",
        [
            { opcode: Opcodes.SCONST, string: typed },
            { opcode: Opcodes.INVOKE, int: EMOTE_SHORTCUT_SCRIPT_ID },
            { opcode: Opcodes.RETURN, int: 0 },
        ],
        { int: 0, obj: 0 },
    );
    vm.execute(caller);
    return vm.intStack[vm.intStackSize - 1];
}

// A real emote still gets its shortcut.
assert.equal(askEmoteScript("dance"), 1, "an exact emote reaches the emote script");
assert.equal(askEmoteScript("run"), 1, "::run is still the Run emote");

// A command that merely starts with an emote name is left alone.
assert.equal(askEmoteScript("runes"), 0, "::runes is a command, not the Run emote");
assert.equal(askEmoteScript("noclip"), 0, "::noclip is a command, not the No emote");
assert.equal(askEmoteScript("presets"), 0);

// The prefix the chat script strips is not part of the name.
assert.equal(isEmoteShortcut("::dance"), true);
assert.equal(isEmoteShortcut("!YES"), true);
assert.equal(isEmoteShortcut(" sit "), true);
assert.equal(isEmoteShortcut("::runes"), false);
assert.equal(isEmoteShortcut(""), false);

console.log("emote-shortcut-gate.test.ts: all tests passed");
