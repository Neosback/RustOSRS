export function browserHostOrigin(): string {
    const value = new URLSearchParams(window.location.search).get("browser-host-origin");
    if (!value) return window.location.origin;
    try { return new URL(value).origin; } catch { return window.location.origin; }
}

export function browserHostWindow(): Window | null {
    if (window.opener && !window.opener.closed) return window.opener;
    return window.parent !== window ? window.parent : null;
}
