export function resolveTradeActionQuantity(
    optionKey: string,
    available: number,
): number | undefined {
    const maximum = Math.max(0, Math.min(2_147_483_647, Math.floor(available)));
    if (maximum <= 0) return undefined;

    // The native trade-offer widget calls its one-item action "Remove"
    // (without the "-1" suffix used by its larger quantity actions).
    if (optionKey === "remove" || optionKey === "offer") return 1;
    if (optionKey.endsWith("all")) return maximum;
    if (optionKey.endsWith("10")) return Math.min(10, maximum);
    if (optionKey.endsWith("5")) return Math.min(5, maximum);
    if (optionKey.endsWith("1")) return 1;
    if (!optionKey.endsWith("x")) return undefined;

    // Offer-X/Remove-X is handled by OsrsClient's native chatbox count dialog.
    // Never fall back to a browser prompt here.
    return undefined;
}
