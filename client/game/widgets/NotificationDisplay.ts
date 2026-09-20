import type { Cs2Vm } from "../../rs/cs2/Cs2Vm";
import type { WidgetManager } from "../../widgets/WidgetManager";

export type NotificationDisplayDeps = {
    getWidgetManager: () => WidgetManager | undefined;
    getCs2Vm: () => Cs2Vm | undefined;
    triggerInitialVarTransmitForGroup: (groupId: number) => void;
};

/**
 * Mounts notification_display (interface 660) into the toplevel notifications container.
 */
export class NotificationDisplay {
    constructor(private readonly deps: NotificationDisplayDeps) {}

    /**
     * Ensure notification_display (interface 660) is mounted into the toplevel
     * notifications container for the current root interface.
     */
    ensureNotificationDisplayMounted(rootGroupId?: number): void {
        const widgetManager = this.deps.getWidgetManager();
        if (!widgetManager) return;

        const root = (rootGroupId ?? widgetManager.rootInterface) | 0;
        if (root === -1) return;

        const targetUid = this.findNotificationsContainerUid(root);
        if (targetUid === null) return;

        const mounted = widgetManager.getSubInterface(targetUid);
        if (mounted && (mounted.group | 0) === 660) {
            return;
        }

        widgetManager.openSubInterface(targetUid, 660, 1);
        this.deps.triggerInitialVarTransmitForGroup(660);

        try {
            const NOTIFICATION_INIT = 3349;
            const cs2Vm = this.deps.getCs2Vm();
            const init = cs2Vm?.context?.loadScript?.(NOTIFICATION_INIT);
            if (init) {
                cs2Vm!.run(init, [targetUid], []);
            }
        } catch {
            // Non-fatal: 3343 will still position the notification container.
        }
    }

    /** Find the toplevel notifications container widget UID for a root interface. */
    findNotificationsContainerUid(rootGroupId: number): number | null {
        const widgetManager = this.deps.getWidgetManager();
        if (!widgetManager) return null;
        const instance = widgetManager.getGroup(rootGroupId);
        if (!instance) return null;

        if (rootGroupId === 161 || rootGroupId === 164) {
            const uid = (rootGroupId << 16) | 13;
            if (widgetManager.getWidgetByUid(uid)) return uid;
        } else if (rootGroupId === 548) {
            const uid = (rootGroupId << 16) | 44;
            if (widgetManager.getWidgetByUid(uid)) return uid;
        } else if (rootGroupId === 601) {
            const uid = (rootGroupId << 16) | 17;
            if (widgetManager.getWidgetByUid(uid)) return uid;
        }

        for (const w of instance.widgetsByUid.values()) {
            if (!w || ((w.type ?? 0) | 0) !== 0) continue;
            if (((w.rawWidth ?? 0) | 0) !== 178) continue;
            if (((w.rawHeight ?? 0) | 0) !== 100) continue;
            if (((w.rawX ?? 0) | 0) !== 0) continue;
            if (((w.rawY ?? 0) | 0) !== 10) continue;
            if (((w.xPositionMode ?? 0) | 0) !== 1) continue;
            const yMode = (w.yPositionMode ?? 0) | 0;
            if (yMode !== 0 && yMode !== 2) continue;
            return (w.uid ?? 0) | 0;
        }

        return null;
    }
}
