import { state } from "../state";
import type { SpotAnimationPayload } from "../types";

export function subscribeSpot(cb: (payload: SpotAnimationPayload) => void): () => void {
    state.spotListeners.add(cb);
    return () => state.spotListeners.delete(cb);
}

export function subscribeSound(
    cb: (payload: {
        soundId: number;
        x?: number;
        y?: number;
        level?: number;
        /** Number of times to play (1 = once). */
        loops?: number;
        /** Delay in client cycles (20ms each). */
        delay?: number;
        /** Radius in tiles (0-31): distance at which the sound becomes silent. */
        radius?: number;
        /** Full volume up to (attenuation & 31) - 1 tiles, then fades to radius. */
        attenuation?: number;
    }) => void,
): () => void {
    state.soundListeners.add(cb);
    return () => state.soundListeners.delete(cb);
}

export function subscribePlaySong(
    cb: (payload: {
        trackId: number;
        fadeOutDelay?: number;
        fadeOutDuration?: number;
        fadeInDelay?: number;
        fadeInDuration?: number;
    }) => void,
): () => void {
    state.playSongListeners.add(cb);
    return () => state.playSongListeners.delete(cb);
}

export function subscribePlayJingle(
    cb: (payload: { jingleId: number; delay?: number }) => void,
): () => void {
    state.playJingleListeners.add(cb);
    return () => state.playJingleListeners.delete(cb);
}
