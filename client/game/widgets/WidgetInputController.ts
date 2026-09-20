import type { WidgetManager } from "../../widgets/WidgetManager";
import type { InputManager } from "../InputManager";
import type { WidgetInteractionController } from "./WidgetInteractionController";
import {
    isQuestListScrollbarWidget,
    processQuestListScrollbarInput,
} from "./input/questListScrollbarInput";
import { shouldSkipWidgetClickInput } from "./input/widgetClickGuard";
import { processWidgetClickInput } from "./input/widgetClickInput";
import { processWidgetDragInput } from "./input/widgetDragInput";
import { processWidgetHoldInput } from "./input/widgetHoldInput";
import { processWidgetHoverInput } from "./input/widgetHoverInput";
import { processWidgetIf1ScrollbarInput } from "./input/widgetIf1ScrollbarInput";
import { buildWidgetInputFrame } from "./input/widgetInputSetup";
import {
    type WidgetInputControllerDeps,
    type WidgetInputState,
    createWidgetInputState,
} from "./input/widgetInputTypes";
import { processWidgetKeyboardInput } from "./input/widgetKeyboardInput";
import { processWidgetMenuWheelInput } from "./input/widgetMenuWheelInput";
import { processWidgetMinimapWheelInput } from "./input/widgetMinimapWheelInput";
import { createPrimaryWidgetActionResolver } from "./input/widgetPrimaryAction";
import { processWidgetReleaseInput } from "./input/widgetReleaseInput";
import { processWidgetScrollWheelInput } from "./input/widgetScrollWheelInput";

export type { WidgetInputControllerDeps } from "./input/widgetInputTypes";

/** Per-frame widget hover/scroll/click/drag/keyboard input extracted from OsrsClient. */
export class WidgetInputController {
    private readonly state: WidgetInputState = createWidgetInputState();

    constructor(private readonly deps: WidgetInputControllerDeps) {}

    handleUiInput(): void {
        const input = this.deps.getInputManager();
        const widgetManager = this.deps.getWidgetManager();
        const widgetInteraction = this.deps.getWidgetInteraction();

        const frame = buildWidgetInputFrame(
            this.deps,
            this.state,
            input,
            widgetManager,
            widgetInteraction,
        );
        if (!frame) return;

        const transmitCycles = this.deps.getTransmitCycles();
        const hoverCycle = transmitCycles.cycleCntr | 0;
        if (this.state.lastHoverListenerCycle !== hoverCycle) {
            this.state.lastHoverListenerCycle = hoverCycle;
            processWidgetHoverInput(this.deps, this.state, frame, widgetManager, widgetInteraction);
        }

        processWidgetMenuWheelInput(this.deps, frame);
        processWidgetMinimapWheelInput(this.deps, frame, widgetManager, widgetInteraction);
        processWidgetIf1ScrollbarInput(
            this.deps,
            this.state,
            frame,
            widgetManager,
            widgetInteraction,
        );
        processQuestListScrollbarInput(frame, widgetManager, widgetInteraction);
        processWidgetScrollWheelInput(this.deps, frame, widgetManager, widgetInteraction);

        if (shouldSkipWidgetClickInput(this.deps, frame)) return;

        const isNewClick = input.leftClickX !== -1 && input.leftClickY !== -1;
        const isHolding = input.isDragging();
        this.deps
            .getWorldMap()
            .handleWorldMapDragInput(frame.hits, frame.mx, frame.my, isNewClick, isHolding);

        const getPrimaryWidgetAction = createPrimaryWidgetActionResolver(
            this.deps,
            input,
            widgetManager,
            widgetInteraction,
        );

        processWidgetClickInput(
            this.deps,
            this.state,
            frame,
            widgetManager,
            widgetInteraction,
            getPrimaryWidgetAction,
            isNewClick,
        );
        // The quest list has a dedicated scroll controller. Its cached
        // scrollbar thumb must not become a generic draggable widget.
        if (!isQuestListScrollbarWidget(widgetInteraction.clickedWidget, widgetManager)) {
            processWidgetDragInput(this.deps, frame, widgetManager, widgetInteraction, isHolding);
        }
        processWidgetHoldInput(this.deps, frame, widgetInteraction, isHolding, isNewClick);
        processWidgetReleaseInput(
            this.deps,
            frame,
            widgetManager,
            widgetInteraction,
            getPrimaryWidgetAction,
            isHolding,
        );
        processWidgetKeyboardInput(this.deps, frame, widgetManager);
    }
}
