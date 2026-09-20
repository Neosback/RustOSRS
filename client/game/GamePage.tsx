import { Bzip2 } from "../rs/compression/Bzip2";
import { Gzip } from "../rs/compression/Gzip";
import { disposeServerConnection, initServerConnection } from "../network/ServerConnection";
import { installUiDiagnostic } from "../ui/UiScaleDiagnostic";
import OsrsClientApp from "./OsrsClientApp";

declare const module: any;

Bzip2.initWasm();
Gzip.initWasm();
installUiDiagnostic();

try {
    const params = new URLSearchParams(window.location.search);
    if (params.has("debugResize")) {
        (window as any).__RESIZE_DEBUG__ = true;
        console.log("[resize] debug enabled via ?debugResize");
    }
} catch {}

try {
    if (typeof module !== "undefined" && module?.hot) {
        module.hot.addStatusHandler((status: string) => {
            if (status === "prepare") {
                try { disposeServerConnection("hmr prepare"); } catch {}
            } else if (status === "idle") {
                try { initServerConnection(); } catch {}
            }
        });
    }
} catch {}

export default function GamePage() {
    return <OsrsClientApp />;
}
