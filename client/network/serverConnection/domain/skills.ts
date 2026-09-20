import type { SkillsServerPayload } from "../types";
import type { SkillsUpdateEvent } from "../types/sync";
import { state } from "../state";
import { sanitizeSkillEntry } from "../utils/sanitize";

export function emitSkills(payload: SkillsServerPayload): void {
    if (!state.lastSkillsState || payload.kind === "snapshot") {
        state.lastSkillsState = {
            totalLevel: Number(payload.totalLevel) || 0,
            combatLevel: Number(payload.combatLevel) || 0,
            byId: new Map(),
        };
    } else {
        state.lastSkillsState.totalLevel = Number(payload.totalLevel) || 0;
        state.lastSkillsState.combatLevel = Number(payload.combatLevel) || 0;
    }

    const changed: import("../types").SkillEntryMessage[] = [];
    for (const raw of payload.skills || []) {
        const entry = sanitizeSkillEntry(raw);
        state.lastSkillsState.byId.set(entry.id, entry);
        changed.push({ ...entry });
    }

    const event: SkillsUpdateEvent = {
        kind: payload.kind,
        totalLevel: state.lastSkillsState.totalLevel,
        combatLevel: state.lastSkillsState.combatLevel,
        skills:
            payload.kind === "snapshot"
                ? Array.from(state.lastSkillsState.byId.values()).map((entry) => ({ ...entry }))
                : changed,
    };

    for (const listener of state.skillsListeners) {
        try {
            listener(event);
        } catch (err) {
            console.warn("skills listener error", err);
        }
    }
}

export function emitPlayerSync(frame: import("../../../game/sync/PlayerSyncTypes").PlayerSyncFrame): void {
    for (const listener of state.playerSyncListeners) {
        try {
            listener(frame);
        } catch (err) {
            console.warn("player sync listener error", err);
        }
    }
}
