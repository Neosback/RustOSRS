import {
    QUEST_LIST_STATUS_COMPLETE,
    QUEST_LIST_STATUS_IN_PROGRESS,
    type QuestListWidgetGroup,
} from "../../common/ui/questList";
import { FONT_BOLD_12, FONT_PLAIN_11 } from "../../ui/fonts";
import type { WidgetManager } from "../WidgetManager";
import type { WidgetNode } from "../WidgetNode";

const QUEST_LIST_GROUP_ID = 399;
const QUEST_LIST_LIST_CHILD_ID = 7;
const QUEST_LIST_TEXT_CONTAINER_CHILD_ID = 6;
const QUEST_LIST_SCROLLBAR_CHILD_ID = 5;

const QUEST_LIST_LIST_UID = (QUEST_LIST_GROUP_ID << 16) | QUEST_LIST_LIST_CHILD_ID;
const QUEST_LIST_TEXT_CONTAINER_UID =
    (QUEST_LIST_GROUP_ID << 16) | QUEST_LIST_TEXT_CONTAINER_CHILD_ID;
const QUEST_LIST_SCROLLBAR_UID = (QUEST_LIST_GROUP_ID << 16) | QUEST_LIST_SCROLLBAR_CHILD_ID;

const HEADER_HEIGHT = 18;
const HEADER_ADVANCE = 25;
const HEADER_Y_OFFSET = 7;
const ROW_LINE_HEIGHT = 11;
const ROW_PADDING = 5;
const ROW_HEIGHT = ROW_LINE_HEIGHT + ROW_PADDING;
const ROW_EXTRA_BOTTOM = 5;

const COLOR_HEADER = 0xff981f;
const COLOR_IN_PROGRESS = 0xffff00;
const COLOR_NOT_STARTED = 0xff0000;
const COLOR_COMPLETE = 0x00ff00;
const COLOR_HOVER = 0xffffff;

function resolveQuestColor(status: number): number {
    if ((status | 0) === QUEST_LIST_STATUS_COMPLETE) return COLOR_COMPLETE;
    if ((status | 0) === QUEST_LIST_STATUS_IN_PROGRESS) return COLOR_IN_PROGRESS;
    return COLOR_NOT_STARTED;
}

function createDynamicTextWidget(
    widgetManager: WidgetManager,
    parent: WidgetNode,
    slot: number,
    overrides: Partial<WidgetNode>,
): WidgetNode {
    const uid = widgetManager.allocateDynamicUid(QUEST_LIST_GROUP_ID);
    return {
        uid,
        id: parent.uid,
        parentUid: parent.uid,
        groupId: parent.groupId,
        fileId: -1,
        type: 4,
        contentType: 0,
        childIndex: slot | 0,
        isIf3: true,
        hidden: false,
        isHidden: false,
        children: null,
        x: 0,
        y: 0,
        width: 0,
        height: 0,
        rawX: 0,
        rawY: 0,
        rawWidth: 0,
        rawHeight: 0,
        widthMode: 0,
        heightMode: 0,
        xPositionMode: 0,
        yPositionMode: 0,
        scrollX: 0,
        scrollY: 0,
        scrollWidth: 0,
        scrollHeight: 0,
        itemId: -1,
        itemQuantity: 0,
        isDraggable: false,
        actions: [],
        rootIndex: -1,
        cycle: -1,
        modelFrame: 0,
        modelFrameCycle: 0,
        aspectWidth: 1,
        aspectHeight: 1,
        color: 0,
        textColor: 0,
        text: "",
        textShadow: true,
        textShadowed: true,
        fontId: FONT_PLAIN_11,
        xTextAlignment: 0,
        yTextAlignment: 1,
        lineHeight: 0,
        spriteId: -1,
        spriteId2: -1,
        params: new Map(),
        ...overrides,
    };
}

function clearDynamicChildren(widgetManager: WidgetManager, parent: WidgetNode): void {
    if (!Array.isArray(parent.children)) {
        parent.children = [];
        widgetManager.invalidateDynamicChildrenCache(parent);
        return;
    }

    for (const child of parent.children) {
        if (child) {
            widgetManager.unregisterWidgetTree(child);
        }
    }
    parent.children = [];
    widgetManager.invalidateDynamicChildrenCache(parent);
}

function registerChild(widgetManager: WidgetManager, parent: WidgetNode, child: WidgetNode): void {
    if (!Array.isArray(parent.children)) parent.children = [];
    const slot = Math.max(0, child.childIndex ?? 0);
    while (parent.children.length <= slot) parent.children.push(null);
    parent.children[slot] = child;
    widgetManager.registerWidget(child);
}

export function applyQuestListWidgetGroups(
    widgetManager: WidgetManager,
    groups: QuestListWidgetGroup[],
): void {
    const list = widgetManager.getWidgetByUid(QUEST_LIST_LIST_UID);
    if (!list) return;

    widgetManager.setServerOwnedWidget(QUEST_LIST_LIST_UID, true);
    widgetManager.setServerOwnedWidget(QUEST_LIST_TEXT_CONTAINER_UID, true);
    widgetManager.setServerOwnedWidget(QUEST_LIST_SCROLLBAR_UID, true);
    clearDynamicChildren(widgetManager, list);

    let y = 0;
    for (const group of Array.isArray(groups) ? groups : []) {
        const title = String(group.title ?? "").trim();
        const quests = Array.isArray(group.quests) ? group.quests : [];
        if (quests.length === 0) continue;

        if (title.length > 0) {
            const headerSlot = Math.max(0, (quests[0]?.slot ?? 1) - 1);
            registerChild(
                widgetManager,
                list,
                createDynamicTextWidget(widgetManager, list, headerSlot, {
                    rawX: 0,
                    rawY: y + HEADER_Y_OFFSET,
                    rawWidth: 0,
                    rawHeight: HEADER_HEIGHT,
                    widthMode: 1,
                    heightMode: 0,
                    xPositionMode: 1,
                    yPositionMode: 0,
                    text: title,
                    fontId: FONT_BOLD_12,
                    textColor: COLOR_HEADER,
                    color: COLOR_HEADER,
                    xTextAlignment: 0,
                    yTextAlignment: 0,
                    actions: [],
                }),
            );
            y += HEADER_ADVANCE;
        }

        for (const quest of quests) {
            const color = resolveQuestColor(quest.status);
            registerChild(
                widgetManager,
                list,
                createDynamicTextWidget(widgetManager, list, quest.slot, {
                    rawX: 0,
                    rawY: y,
                    rawWidth: 0,
                    rawHeight: ROW_HEIGHT,
                    widthMode: 1,
                    heightMode: 0,
                    xPositionMode: 1,
                    yPositionMode: 0,
                    text: quest.displayName,
                    fontId: FONT_PLAIN_11,
                    textColor: color,
                    color,
                    mouseOverColor: COLOR_HOVER,
                    xTextAlignment: 0,
                    yTextAlignment: 1,
                    lineHeight: ROW_LINE_HEIGHT,
                    actions: ["", "Read journal:", "", "", "", "Pin journal:"],
                    opBase: `<col=ff9040>${quest.displayName}</col>`,
                }),
            );
            y += ROW_HEIGHT;
        }
    }

    const textContainer = widgetManager.getWidgetByUid(QUEST_LIST_TEXT_CONTAINER_UID);
    // The enclosing text pane is the actual viewport. The row list can retain
    // an old content height across a tab refresh, so using list.height here can
    // incorrectly hide the scrollbar after an overflowed list is rebuilt.
    const enclosingHeight = textContainer?.height ?? 0;
    const viewportHeight = Math.max(0, (enclosingHeight > 0 ? enclosingHeight : list.height) | 0);
    const contentHeight = Math.max(viewportHeight, y + ROW_EXTRA_BOTTOM);
    list.rawHeight = viewportHeight;
    list.height = viewportHeight;
    list.scrollHeight = contentHeight;
    list.scrollY = Math.min(list.scrollY | 0, Math.max(0, contentHeight - viewportHeight));

    if (textContainer) {
        textContainer.scrollHeight = list.scrollHeight;
        textContainer.scrollY = Math.min(
            textContainer.scrollY | 0,
            Math.max(0, textContainer.scrollHeight - (textContainer.height | 0)),
        );
        widgetManager.invalidateWidget(textContainer, "quest-list");
    }

    const scrollbar = widgetManager.getWidgetByUid(QUEST_LIST_SCROLLBAR_UID);
    if (scrollbar) {
        // The list (399:7) is the component that owns the dynamic rows and
        // therefore the scroll position. Make the link explicit so the custom
        // quest list does not depend on a cache script's inferred linkage.
        (
            scrollbar as WidgetNode & { scrollBarTargetUid?: number; scrollBarAxis?: "y" }
        ).scrollBarTargetUid = list.uid;
        (
            scrollbar as WidgetNode & { scrollBarTargetUid?: number; scrollBarAxis?: "y" }
        ).scrollBarAxis = "y";
        scrollbar.isHidden = contentHeight <= viewportHeight;
        scrollbar.hidden = scrollbar.isHidden;
        widgetManager.invalidateWidget(scrollbar, "quest-list");
    }

    widgetManager.invalidateDynamicChildrenCache(list);
    widgetManager.invalidateWidget(list, "quest-list");
}
