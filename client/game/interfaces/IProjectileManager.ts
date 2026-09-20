import type { ProjectileLaunch } from "../../common/projectiles/ProjectileLaunch";

export interface IProjectileManager {
    launch(launch: ProjectileLaunch): number;
    remove(id: number): void;
    clear(): void;
    getCount(): number;
    update(deltaTimeMs?: number): void;
}
