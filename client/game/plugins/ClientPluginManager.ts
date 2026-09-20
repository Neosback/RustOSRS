import type { Camera } from "../Camera";
import type { InputManager } from "../InputManager";
import type { DrawCall, Program } from "picogl";
import type { ProgramSource } from "../../render/shaders/ShaderUtil";
import type { WebGLOsrsRenderer } from "../../render/WebGLOsrsRenderer";

export type CameraInputContext = {
    camera: Camera;
    input: InputManager;
    deltaTime: number;
};

export type CameraFollowContext = {
    camera: Camera;
    playerX: number;
    playerY?: number;
    playerZ: number;
};

export interface ClientPlugin {
    transformSceneProgram?(source: ProgramSource): ProgramSource;
    sceneProgramsReady?(renderer: WebGLOsrsRenderer, programs: Program[]): void;
    beforeSceneRender?(renderer: WebGLOsrsRenderer, drawActors: () => void): void;
    configureSceneDrawCall?(renderer: WebGLOsrsRenderer, drawCall: DrawCall): void;
    disposeRenderer?(renderer: WebGLOsrsRenderer): void;
    handleCameraKeys?(context: CameraInputContext): boolean;
    handleCameraMouse?(context: CameraInputContext): boolean;
    handleCameraScroll?(context: CameraInputContext): boolean;
    updateInteractionPointer?(camera: Camera): void;
    handleCameraFollow?(context: CameraFollowContext): boolean;
    shouldKeepWorldMenuOpen?(): boolean;
}

export class ClientPluginManager {
    private readonly plugins: ClientPlugin[] = [];

    add(plugin: ClientPlugin): void {
        this.plugins.push(plugin);
    }

    transformSceneProgram(source: ProgramSource): ProgramSource {
        for (const plugin of this.plugins) source = plugin.transformSceneProgram?.(source) ?? source;
        return source;
    }

    sceneProgramsReady(renderer: WebGLOsrsRenderer, programs: Program[]): void {
        for (const plugin of this.plugins) plugin.sceneProgramsReady?.(renderer, programs);
    }

    beforeSceneRender(renderer: WebGLOsrsRenderer, drawActors: () => void): void {
        for (const plugin of this.plugins) plugin.beforeSceneRender?.(renderer, drawActors);
    }

    configureSceneDrawCall(renderer: WebGLOsrsRenderer, drawCall: DrawCall): void {
        for (const plugin of this.plugins) plugin.configureSceneDrawCall?.(renderer, drawCall);
    }

    disposeRenderer(renderer: WebGLOsrsRenderer): void {
        for (const plugin of this.plugins) plugin.disposeRenderer?.(renderer);
    }

    handleCameraKeys(context: CameraInputContext): boolean {
        return this.plugins.some((plugin) => plugin.handleCameraKeys?.(context) === true);
    }

    handleCameraMouse(context: CameraInputContext): boolean {
        return this.plugins.some((plugin) => plugin.handleCameraMouse?.(context) === true);
    }

    handleCameraScroll(context: CameraInputContext): boolean {
        return this.plugins.some((plugin) => plugin.handleCameraScroll?.(context) === true);
    }

    updateInteractionPointer(camera: Camera): void {
        for (const plugin of this.plugins) plugin.updateInteractionPointer?.(camera);
    }

    handleCameraFollow(context: CameraFollowContext): boolean {
        return this.plugins.some((plugin) => plugin.handleCameraFollow?.(context) === true);
    }

    shouldKeepWorldMenuOpen(): boolean {
        return this.plugins.some((plugin) => plugin.shouldKeepWorldMenuOpen?.() === true);
    }

}
