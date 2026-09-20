export function decodeBase64(input: string): Uint8Array {
    if (typeof input !== "string" || input.length === 0) return new Uint8Array();
    if (typeof atob === "function") {
        const binary = atob(input);
        const len = binary.length | 0;
        const bytes = new Uint8Array(len);
        for (let i = 0; i < len; i++) bytes[i] = binary.charCodeAt(i) & 0xff;
        return bytes;
    }
    try {
        const bufferCtor: any = (globalThis as any).Buffer;
        if (bufferCtor?.from) {
            return new Uint8Array(bufferCtor.from(input, "base64"));
        }
    } catch {}
    const fallback: number[] = [];
    for (let i = 0; i < input.length; i++) fallback.push(input.charCodeAt(i) & 0xff);
    return new Uint8Array(fallback);
}
