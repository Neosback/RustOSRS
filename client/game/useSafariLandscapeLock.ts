import { useEffect, useState } from "react";

import { checkMobile, shouldFreezeViewportForVirtualKeyboard } from "../common/utils/DeviceUtil";

interface MobileLandscapeLockState {
    enabled: boolean;
    rotated: boolean;
}

function readViewportSize(): { width: number; height: number } {
    if (typeof window === "undefined") {
        return { width: 0, height: 0 };
    }

    const viewport = window.visualViewport;
    if (viewport) {
        return {
            width: Math.max(1, Math.round(viewport.width)),
            height: Math.max(1, Math.round(viewport.height)),
        };
    }

    return {
        width: Math.max(1, Math.round(window.innerWidth)),
        height: Math.max(1, Math.round(window.innerHeight)),
    };
}

/**
 * Force a landscape layout on phones (iPhone + Android).
 * When the device is physically portrait, CSS-rotates the app 90° and remaps
 * input (via documentElement dataset flags). Also tries Screen Orientation lock
 * when the browser allows it (often after a user gesture / in installed PWAs).
 *
 * Dataset attribute names keep the historical `iosSafariForceLandscape*` keys
 * so InputManager / DeviceUtil continue to work unchanged.
 */
export function useSafariLandscapeLock(enabled: boolean = true): MobileLandscapeLockState {
    const [rotated, setRotated] = useState(false);
    const isPhone = checkMobile();
    const active = enabled && isPhone;

    useEffect(() => {
        if (!active || typeof window === "undefined" || typeof document === "undefined") {
            setRotated(false);
            return;
        }

        const root = document.documentElement;
        let rafId: number | undefined;
        let lastApplied: { width: number; height: number } | undefined;

        const applyViewportMetrics = () => {
            const { width, height } = readViewportSize();
            // While the on-screen keyboard is open the visual viewport only reflects the
            // sliver above the keyboard. Recomputing the forced-landscape size (and the
            // rotation flag) from it collapses the app and can un-rotate it mid-typing,
            // so keep the pre-keyboard metrics until the keyboard closes.
            if (shouldFreezeViewportForVirtualKeyboard(lastApplied, { width, height })) {
                return;
            }
            lastApplied = { width, height };
            root.style.setProperty("--ios-safari-vw", `${width}px`);
            root.style.setProperty("--ios-safari-vh", `${height}px`);
            root.style.setProperty("--ios-safari-landscape-w", `${Math.max(width, height)}px`);
            root.style.setProperty("--ios-safari-landscape-h", `${Math.min(width, height)}px`);
            const shouldRotate = height > width;
            setRotated(shouldRotate);
            root.dataset.iosSafariForceLandscape = "1";
            root.dataset.iosSafariForceLandscapeRotated = shouldRotate ? "1" : "0";
        };

        const scheduleApply = () => {
            if (rafId !== undefined) {
                cancelAnimationFrame(rafId);
            }
            rafId = requestAnimationFrame(() => {
                rafId = undefined;
                applyViewportMetrics();
            });
        };

        const tryLockLandscape = () => {
            try {
                const orientation = window.screen?.orientation as
                    | (ScreenOrientation & {
                          lock?: (orientation: "landscape") => Promise<void>;
                      })
                    | undefined;
                if (!orientation || typeof orientation.lock !== "function") {
                    return;
                }
                orientation.lock("landscape").catch(() => {});
            } catch {}
        };

        scheduleApply();
        tryLockLandscape();

        window.addEventListener("resize", scheduleApply);
        window.addEventListener("orientationchange", scheduleApply);
        window.addEventListener("pageshow", scheduleApply);
        document.addEventListener("visibilitychange", scheduleApply);
        // Re-evaluate when focus leaves an editable element (keyboard closing).
        document.addEventListener("focusout", scheduleApply);
        window.addEventListener("touchstart", tryLockLandscape, { passive: true });
        window.addEventListener("click", tryLockLandscape);

        const viewport = window.visualViewport;
        viewport?.addEventListener("resize", scheduleApply);
        viewport?.addEventListener("scroll", scheduleApply);

        return () => {
            window.removeEventListener("resize", scheduleApply);
            window.removeEventListener("orientationchange", scheduleApply);
            window.removeEventListener("pageshow", scheduleApply);
            document.removeEventListener("visibilitychange", scheduleApply);
            document.removeEventListener("focusout", scheduleApply);
            window.removeEventListener("touchstart", tryLockLandscape);
            window.removeEventListener("click", tryLockLandscape);
            viewport?.removeEventListener("resize", scheduleApply);
            viewport?.removeEventListener("scroll", scheduleApply);
            if (rafId !== undefined) {
                cancelAnimationFrame(rafId);
            }
            root.style.removeProperty("--ios-safari-vw");
            root.style.removeProperty("--ios-safari-vh");
            root.style.removeProperty("--ios-safari-landscape-w");
            root.style.removeProperty("--ios-safari-landscape-h");
            delete root.dataset.iosSafariForceLandscape;
            delete root.dataset.iosSafariForceLandscapeRotated;
        };
    }, [active]);

    return {
        enabled: active,
        rotated,
    };
}
