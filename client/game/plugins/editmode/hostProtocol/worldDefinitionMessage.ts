export const WORLD_DEFINITION_MESSAGE = "elvarg:world-definition";
export const WORLD_DEFINITION_REQUEST_MESSAGE = "elvarg:world-definition-request";
export type WorldDefinitionMessage = { type: typeof WORLD_DEFINITION_MESSAGE; contents: string };
export type WorldDefinitionRequestMessage = { type: typeof WORLD_DEFINITION_REQUEST_MESSAGE };
export function isWorldPluginEnabled(contents: string, pluginName: string): boolean {
    const world = JSON.parse(contents) as { disabledPlugins?: unknown };
    return !Array.isArray(world.disabledPlugins) || !world.disabledPlugins.includes(pluginName);
}
export function setWorldPluginEnabled(contents: string, pluginName: string, enabled: boolean): string {
    const world = JSON.parse(contents) as { disabledPlugins?: unknown };
    const disabled = Array.isArray(world.disabledPlugins) ? world.disabledPlugins.filter((name) => name !== pluginName) : [];
    if (!enabled) disabled.push(pluginName);
    world.disabledPlugins = disabled;
    return JSON.stringify(world, null, 2) + "\n";
}
