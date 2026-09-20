import { useEffect, useRef } from "react";

import { Renderer } from "../game/render/Renderer";

export interface CanvasProps {
    renderer: Renderer;
}

export function Canvas({ renderer }: CanvasProps): JSX.Element {
    const divRef = useRef<HTMLDivElement>(null);

    useEffect(() => {
        const host = divRef.current;
        if (!host) {
            return;
        }
        let active = true;
        host.appendChild(renderer.canvas);
        renderer.attachResizeObserver();
        requestAnimationFrame(() => renderer.forceResize());

        renderer.initOnce().then(() => {
            if (active) renderer.start();
        });

        return () => {
            active = false;
            renderer.stop();
            if (renderer.canvas.parentNode === host) host.removeChild(renderer.canvas);
        };
    }, [renderer]);

    return (
        <div
            ref={divRef}
            style={{ position: "relative", width: "100%", height: "100%" }}
            tabIndex={0}
        />
    );
}
