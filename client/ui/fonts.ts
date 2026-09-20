// Centralized font ID definitions to avoid magic numbers.
// Names mirror the CS2/cache font assets.
export const FONT_PLAIN_11 = 494; // p11_full
export const FONT_PLAIN_12 = 495; // p12_full
export const FONT_BOLD_12 = 496; // b12_full
export const FONT_FANCY_8 = 497; // q8_full (quest/XP/fancy)
export const FONT_QUILL_8 = 645; // quill8
export const FONT_VERDANA_11 = 1442; // verdana_11pt_regular
export const FONT_VERDANA_13 = 1445; // verdana_13pt_regular
export const FONT_VERDANA_15 = 1447; // verdana_15pt_regular

export type FontName =
    | "p11_full"
    | "p12_full"
    | "b12_full"
    | "q8_full"
    | "quill8"
    | "verdana_11pt_regular"
    | "verdana_13pt_regular"
    | "verdana_15pt_regular";

const FONT_MAP: Record<FontName, number> = {
    p11_full: FONT_PLAIN_11,
    p12_full: FONT_PLAIN_12,
    b12_full: FONT_BOLD_12,
    q8_full: FONT_FANCY_8,
    quill8: FONT_QUILL_8,
    verdana_11pt_regular: FONT_VERDANA_11,
    verdana_13pt_regular: FONT_VERDANA_13,
    verdana_15pt_regular: FONT_VERDANA_15,
};

export function fontId(name: FontName): number {
    return FONT_MAP[name];
}
