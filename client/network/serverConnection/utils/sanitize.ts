import { SkillId } from "../../../rs/skill/skills";
import { BANK_SLOT_COUNT_FALLBACK, INVENTORY_SLOT_COUNT } from "../constants";
import type {
    BankSlotMessage,
    InventorySlotMessage,
    ShopStockEntryMessage,
    SkillEntryMessage,
    SmithingOptionMessage,
    SpellResultPayload,
    TradeOfferEntryMessage,
    TradePartyMessage,
    TradePartyViewState,
} from "../types";

export function sanitizeInventorySlotMessage(raw: any): InventorySlotMessage {
    const slot = typeof raw?.slot === "number" ? raw.slot | 0 : 0;
    const itemId = typeof raw?.itemId === "number" ? raw.itemId | 0 : -1;
    const quantity = typeof raw?.quantity === "number" ? raw.quantity | 0 : 0;
    return {
        slot: Math.max(0, Math.min(INVENTORY_SLOT_COUNT - 1, slot)),
        itemId,
        quantity,
    };
}

export function sanitizeBankSlotMessage(raw: any, capacityHint?: number): InventorySlotMessage {
    const slot = typeof raw?.slot === "number" ? raw.slot | 0 : 0;
    const itemId = typeof raw?.itemId === "number" ? raw.itemId | 0 : -1;
    const quantity = typeof raw?.quantity === "number" ? raw.quantity | 0 : 0;
    const maxHint =
        typeof capacityHint === "number" && capacityHint > 0 ? Math.max(1, capacityHint | 0) : 0;
    const max = Math.max(BANK_SLOT_COUNT_FALLBACK, maxHint);
    return {
        slot: Math.max(0, Math.min(max - 1, slot)),
        itemId,
        quantity,
    };
}

export function sanitizeShopStockEntryMessage(raw: any): ShopStockEntryMessage {
    const slot = Number(raw?.slot);
    const itemId = Number(raw?.itemId);
    const quantity = Number(raw?.quantity);
    const defaultQuantity = Number(raw?.defaultQuantity);
    const priceEach = Number(raw?.priceEach);
    const sellPrice = Number(raw?.sellPrice);
    const normalized: ShopStockEntryMessage = {
        slot: Number.isFinite(slot) ? Math.max(0, slot | 0) : 0,
        itemId: Number.isFinite(itemId) ? itemId | 0 : -1,
        quantity: Number.isFinite(quantity) ? Math.max(0, quantity | 0) : 0,
    };
    if (Number.isFinite(defaultQuantity)) {
        normalized.defaultQuantity = Math.max(0, (defaultQuantity as number) | 0);
    }
    if (Number.isFinite(priceEach) && (priceEach as number) >= 0) {
        normalized.priceEach = Math.max(0, (priceEach as number) | 0);
    }
    if (Number.isFinite(sellPrice) && (sellPrice as number) >= 0) {
        normalized.sellPrice = Math.max(0, (sellPrice as number) | 0);
    }
    return normalized;
}

export function sanitizeSmithingOption(raw: any, fallbackIdx: number): SmithingOptionMessage {
    const recipeId =
        typeof raw?.recipeId === "string" && raw.recipeId.trim().length > 0
            ? raw.recipeId
            : `recipe_${fallbackIdx | 0}`;
    const name = typeof raw?.name === "string" && raw.name.trim().length > 0 ? raw.name : recipeId;
    const levelRaw = Number(raw?.level);
    const itemIdRaw = Number(raw?.itemId);
    const qtyRaw = Number(raw?.outputQuantity);
    const availableRaw = Number(raw?.available);
    const xpRaw = Number(raw?.xp);
    const requiresHammer = !!raw?.requiresHammer;
    const hasHammer = raw?.hasHammer === undefined ? true : !!raw?.hasHammer;
    return {
        recipeId,
        name,
        level: Number.isFinite(levelRaw) ? Math.max(1, levelRaw | 0) : 1,
        itemId: Number.isFinite(itemIdRaw) ? (itemIdRaw as number) | 0 : -1,
        outputQuantity: Number.isFinite(qtyRaw) ? Math.max(1, qtyRaw | 0) : 1,
        available: Number.isFinite(availableRaw) ? Math.max(0, availableRaw | 0) : 0,
        canMake: !!raw?.canMake,
        xp: Number.isFinite(xpRaw) && xpRaw > 0 ? xpRaw : undefined,
        ingredientsLabel:
            typeof raw?.ingredientsLabel === "string" && raw.ingredientsLabel.trim().length > 0
                ? raw.ingredientsLabel
                : undefined,
        mode: raw?.mode === "forge" ? "forge" : "smelt",
        barItemId: Number.isFinite(raw?.barItemId) ? (raw.barItemId as number) | 0 : undefined,
        barCount: Number.isFinite(raw?.barCount) ? (raw.barCount as number) | 0 : undefined,
        requiresHammer,
        hasHammer: requiresHammer ? hasHammer : true,
    };
}

export function sanitizeTradeOfferEntry(raw: any, fallbackSlot: number): TradeOfferEntryMessage {
    const slotRaw = Number(raw?.slot);
    const itemIdRaw = Number(raw?.itemId);
    const quantityRaw = Number(raw?.quantity);
    return {
        slot: Number.isFinite(slotRaw) ? (slotRaw as number) | 0 : fallbackSlot | 0,
        itemId: Number.isFinite(itemIdRaw) ? (itemIdRaw as number) | 0 : -1,
        quantity: Number.isFinite(quantityRaw) ? Math.max(0, (quantityRaw as number) | 0) : 0,
    };
}

export function sanitizeTradePartyMessage(
    raw: TradePartyMessage | undefined,
): TradePartyViewState | undefined {
    if (!raw) return undefined;
    const offers = Array.isArray(raw.offers)
        ? raw.offers.map((entry, idx) => sanitizeTradeOfferEntry(entry, idx))
        : [];
    return {
        playerId: typeof raw.playerId === "number" ? raw.playerId | 0 : undefined,
        name: raw.name ? String(raw.name) : undefined,
        offers,
        accepted: !!raw.accepted,
        confirmAccepted: !!raw.confirmAccepted,
    };
}

export function sanitizeSkillEntry(raw: any): SkillEntryMessage {
    const id = Number(raw?.id) | 0;
    const xp = Math.max(0, Number(raw?.xp) || 0);
    const baseLevel = Math.max(1, Number(raw?.baseLevel) || 1);
    const virtualLevel = Math.max(baseLevel, Number(raw?.virtualLevel) || baseLevel);
    const boost = Number.isFinite(raw?.boost) ? Number(raw.boost) : 0;
    const minCurrent = id === SkillId.Hitpoints || id === SkillId.Prayer ? 0 : 1;
    const fallbackCurrent = baseLevel + boost;
    const rawCurrent = Number(raw?.currentLevel);
    const currentLevel = Math.max(
        minCurrent,
        Number.isFinite(rawCurrent) ? (rawCurrent as number) : fallbackCurrent,
    );
    return {
        id,
        xp,
        baseLevel,
        virtualLevel,
        boost,
        currentLevel,
    };
}

export function sanitizeSpellResult(raw: any): SpellResultPayload {
    const casterId = Number(raw?.casterId) | 0;
    const spellId = Number(raw?.spellId) | 0;
    const outcome = raw?.outcome === "success" ? "success" : "failure";
    const validTargetTypes = new Set(["npc", "player", "loc", "obj", "tile"]);
    const targetTypeRaw = typeof raw?.targetType === "string" ? raw.targetType : "npc";
    const targetType = validTargetTypes.has(targetTypeRaw) ? (targetTypeRaw as any) : "npc";
    const targetIdRaw = Number(raw?.targetId);
    const targetId = Number.isFinite(targetIdRaw) ? targetIdRaw | 0 : undefined;

    const tileRaw = raw?.tile;
    const tile =
        tileRaw && typeof tileRaw === "object"
            ? {
                  x: Number(tileRaw.x) | 0,
                  y: Number(tileRaw.y) | 0,
                  plane:
                      tileRaw.plane !== undefined && Number.isFinite(tileRaw.plane)
                          ? Number(tileRaw.plane) | 0
                          : undefined,
              }
            : undefined;

    const sanitizeRuneDelta = (entry: any): { itemId: number; quantity: number } | undefined => {
        const itemId = Number(entry?.itemId) | 0;
        const quantity = Number(entry?.quantity) || 0;
        if (!Number.isFinite(itemId) || itemId <= 0) return undefined;
        if (!Number.isFinite(quantity) || quantity === 0) return undefined;
        return { itemId, quantity: quantity | 0 };
    };

    const runesConsumed = Array.isArray(raw?.runesConsumed)
        ? (raw.runesConsumed as any[])
              .map((entry) => sanitizeRuneDelta(entry))
              .filter((entry): entry is { itemId: number; quantity: number } => !!entry)
        : undefined;
    const runesRefunded = Array.isArray(raw?.runesRefunded)
        ? (raw.runesRefunded as any[])
              .map((entry) => sanitizeRuneDelta(entry))
              .filter((entry): entry is { itemId: number; quantity: number } => !!entry)
        : undefined;

    const hitDelayRaw = Number(raw?.hitDelay);
    const impactSpotAnimRaw = Number(raw?.impactSpotAnim);
    const castSpotAnimRaw = Number(raw?.castSpotAnim);
    const splashSpotAnimRaw = Number(raw?.splashSpotAnim);
    const damageRaw = Number(raw?.damage);
    const maxHitRaw = Number(raw?.maxHit);
    const accuracyRaw = Number(raw?.accuracy);

    const modifiersRaw = raw?.modifiers;
    const modifiers =
        modifiersRaw && typeof modifiersRaw === "object"
            ? {
                  isAutocast: !!modifiersRaw.isAutocast,
                  defensive: !!modifiersRaw.defensive,
                  queued: !!modifiersRaw.queued,
                  castMode:
                      modifiersRaw.castMode === "autocast" ||
                      modifiersRaw.castMode === "defensive_autocast"
                          ? modifiersRaw.castMode
                          : "manual",
              }
            : undefined;

    const reason =
        typeof raw?.reason === "string" && raw.reason.length > 0
            ? (raw.reason as string)
            : undefined;

    return {
        casterId,
        spellId,
        outcome,
        targetType,
        targetId,
        tile,
        modifiers,
        reason,
        runesConsumed,
        runesRefunded,
        hitDelay: Number.isFinite(hitDelayRaw) ? Math.max(0, hitDelayRaw | 0) : undefined,
        impactSpotAnim: Number.isFinite(impactSpotAnimRaw) ? impactSpotAnimRaw | 0 : undefined,
        castSpotAnim: Number.isFinite(castSpotAnimRaw) ? castSpotAnimRaw | 0 : undefined,
        splashSpotAnim: Number.isFinite(splashSpotAnimRaw) ? splashSpotAnimRaw | 0 : undefined,
        damage: Number.isFinite(damageRaw) ? damageRaw | 0 : undefined,
        maxHit: Number.isFinite(maxHitRaw) ? maxHitRaw | 0 : undefined,
        accuracy: Number.isFinite(accuracyRaw) ? Math.max(0, Math.min(1, accuracyRaw)) : undefined,
    };
}
