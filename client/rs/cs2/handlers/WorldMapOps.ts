/**
 * World map operations
 */
import { packWorldMapCoord, unpackWorldMapCoord } from "../../map/WorldMapArea";
import { Opcodes } from "../Opcodes";
import type { HandlerMap } from "./HandlerTypes";

export function registerWorldMapOps(handlers: HandlerMap): void {
    handlers.set(Opcodes.WORLDMAP_INIT, (ctx) => {
        const plane = ctx.getPlayerPlane?.() ?? 0;
        const x = ctx.getPlayerLocalX?.() ?? 0;
        const y = ctx.getPlayerLocalY?.() ?? 0;
        ctx.worldMapState?.setCurrentMapAreaAndPosition(plane, x, y);
    });

    handlers.set(Opcodes.WORLDMAP_GETMAPNAME, (ctx) => {
        const mapAreaId = ctx.popInt();
        ctx.pushString(ctx.worldMapState?.getMapArea(mapAreaId)?.externalName ?? "");
    });

    handlers.set(Opcodes.WORLDMAP_SETMAP, (ctx) => {
        const mapAreaId = ctx.popInt();
        ctx.worldMapState?.setCurrentMapAreaId(mapAreaId);
    });

    handlers.set(Opcodes.WORLDMAP_GETZOOM, (ctx) => {
        ctx.pushInt(ctx.worldMapState?.zoomPercentage ?? 100);
    });

    handlers.set(Opcodes.WORLDMAP_SETZOOM, (ctx) => {
        const zoom = ctx.popInt();
        ctx.worldMapState?.setZoomPercentage(zoom);
        ctx.widgetManager.invalidateAll();
    });

    handlers.set(Opcodes.WORLDMAP_ISLOADED, (ctx) => {
        ctx.pushInt(ctx.worldMapState?.isLoaded() ? 1 : 0);
    });

    handlers.set(Opcodes.WORLDMAP_JUMPTODISPLAYCOORD, (ctx) => {
        const coord = unpackWorldMapCoord(ctx.popInt());
        ctx.worldMapState?.setWorldMapPositionTarget(coord.x, coord.y);
    });

    handlers.set(Opcodes.WORLDMAP_JUMPTODISPLAYCOORD_INSTANT, (ctx) => {
        const coord = unpackWorldMapCoord(ctx.popInt());
        ctx.worldMapState?.setWorldMapPositionTargetInstant(coord.x, coord.y);
    });

    handlers.set(Opcodes.WORLDMAP_JUMPTOSOURCECOORD, (ctx) => {
        const coord = unpackWorldMapCoord(ctx.popInt());
        ctx.worldMapState?.jumpToSourceCoord(coord.plane, coord.x, coord.y);
    });

    handlers.set(Opcodes.WORLDMAP_JUMPTOSOURCECOORD_INSTANT, (ctx) => {
        const coord = unpackWorldMapCoord(ctx.popInt());
        ctx.worldMapState?.jumpToSourceCoordInstant(coord.plane, coord.x, coord.y);
    });

    handlers.set(Opcodes.WORLDMAP_GETDISPLAYPOSITION, (ctx) => {
        ctx.pushInt(ctx.worldMapState?.displayX ?? -1);
        ctx.pushInt(ctx.worldMapState?.displayY ?? -1);
    });

    handlers.set(Opcodes.WORLDMAP_GETCONFIGORIGIN, (ctx) => {
        const mapAreaId = ctx.popInt();
        ctx.pushInt(ctx.worldMapState?.getMapArea(mapAreaId)?.getOriginPacked() ?? 0);
    });

    handlers.set(Opcodes.WORLDMAP_GETCONFIGSIZE, (ctx) => {
        const mapAreaId = ctx.popInt();
        const mapArea = ctx.worldMapState?.getMapArea(mapAreaId);
        ctx.pushInt(mapArea?.getWidthTiles() ?? 0);
        ctx.pushInt(mapArea?.getHeightTiles() ?? 0);
    });

    handlers.set(Opcodes.WORLDMAP_GETCONFIGBOUNDS, (ctx) => {
        const mapAreaId = ctx.popInt();
        const bounds = ctx.worldMapState?.getMapArea(mapAreaId)?.getBounds();
        ctx.pushInt(bounds?.minX ?? 0);
        ctx.pushInt(bounds?.minY ?? 0);
        ctx.pushInt(bounds?.maxX ?? 0);
        ctx.pushInt(bounds?.maxY ?? 0);
    });

    handlers.set(Opcodes.WORLDMAP_GETCONFIGZOOM, (ctx) => {
        const mapAreaId = ctx.popInt();
        ctx.pushInt(ctx.worldMapState?.getMapArea(mapAreaId)?.zoom ?? -1);
    });

    handlers.set(Opcodes.WORLDMAP_GETDISPLAYCOORD_CURRENT, (ctx) => {
        const coord = ctx.worldMapState?.getDisplayCoord();
        ctx.pushInt(coord?.x ?? -1);
        ctx.pushInt(coord?.y ?? -1);
    });

    handlers.set(Opcodes.WORLDMAP_GETCURRENTMAP, (ctx) => {
        ctx.pushInt(ctx.worldMapState?.getCurrentMapAreaId() ?? -1);
    });

    handlers.set(Opcodes.WORLDMAP_GETDISPLAYCOORD, (ctx) => {
        const packedCoord = ctx.popInt();
        const displayPosition = ctx.worldMapState?.sourceToDisplay(packedCoord);
        ctx.pushInt(displayPosition?.x ?? -1);
        ctx.pushInt(displayPosition?.y ?? -1);
    });

    handlers.set(Opcodes.WORLDMAP_GETSOURCECOORD, (ctx) => {
        const packedCoord = ctx.popInt();
        const coord = ctx.worldMapState?.displayToSource(packedCoord);
        ctx.pushInt(coord ? packWorldMapCoord(coord) : -1);
    });

    handlers.set(Opcodes.WORLDMAP_JUMPTOMAP, (ctx) => {
        const fallback = unpackWorldMapCoord(ctx.popInt());
        const mapAreaId = ctx.popInt();
        const preferred = {
            plane: ctx.getPlayerPlane?.() ?? 0,
            x: ctx.getPlayerLocalX?.() ?? 0,
            y: ctx.getPlayerLocalY?.() ?? 0,
        };
        ctx.worldMapState?.jumpToMapArea(mapAreaId, preferred, fallback, false);
    });

    handlers.set(Opcodes.WORLDMAP_JUMPTOMAP_INSTANT, (ctx) => {
        const fallback = unpackWorldMapCoord(ctx.popInt());
        const mapAreaId = ctx.popInt();
        const preferred = {
            plane: ctx.getPlayerPlane?.() ?? 0,
            x: ctx.getPlayerLocalX?.() ?? 0,
            y: ctx.getPlayerLocalY?.() ?? 0,
        };
        ctx.worldMapState?.jumpToMapArea(mapAreaId, preferred, fallback, true);
    });

    handlers.set(Opcodes.WORLDMAP_COORDINMAP, (ctx) => {
        const coord = ctx.popInt();
        const mapAreaId = ctx.popInt();
        ctx.pushInt(ctx.worldMapState?.coordInMap(mapAreaId, coord) ? 1 : 0);
    });

    handlers.set(Opcodes.WORLDMAP_GETSIZE, (ctx) => {
        ctx.pushInt(ctx.worldMapState?.displayWidth ?? 0);
        ctx.pushInt(ctx.worldMapState?.displayHeight ?? 0);
    });

    handlers.set(Opcodes.WORLDMAP_GETMAP, (ctx) => {
        const coord = unpackWorldMapCoord(ctx.popInt());
        const mapArea = ctx.worldMapState?.mapAreaAtCoord(coord.plane, coord.x, coord.y);
        ctx.pushInt(mapArea?.id ?? -1);
    });

    handlers.set(Opcodes.WORLDMAP_SETMAXFLASHCOUNT, (ctx) => {
        const count = ctx.popInt();
        ctx.worldMapState?.setMaxFlashCount(count);
        ctx.widgetManager.invalidateAll();
    });

    handlers.set(Opcodes.WORLDMAP_RESETMAXFLASHCOUNT, (ctx) => {
        ctx.worldMapState?.resetMaxFlashCount();
        ctx.widgetManager.invalidateAll();
    });

    handlers.set(Opcodes.WORLDMAP_SETCYCLESPERFLASH, (ctx) => {
        const cycles = ctx.popInt();
        ctx.worldMapState?.setCyclesPerFlash(cycles);
        ctx.widgetManager.invalidateAll();
    });

    handlers.set(Opcodes.WORLDMAP_RESETCYCLESPERFLASH, (ctx) => {
        ctx.worldMapState?.resetCyclesPerFlash();
        ctx.widgetManager.invalidateAll();
    });

    handlers.set(Opcodes.WORLDMAP_GETNEARESTICON, (ctx) => {
        const sourceCoord = ctx.popInt();
        const elementId = ctx.popInt();
        ctx.pushInt(ctx.worldMapState?.getNearestIconCoord(elementId, sourceCoord) ?? -1);
    });

    handlers.set(Opcodes.WORLDMAP_PERPETUALFLASH, (ctx) => {
        if (ctx.worldMapState) ctx.worldMapState.perpetualFlash = ctx.popInt() === 1;
        else ctx.intStackSize--;
        ctx.widgetManager.invalidateAll();
    });

    handlers.set(Opcodes.WORLDMAP_FLASHELEMENT, (ctx) => {
        const elementId = ctx.popInt();
        ctx.worldMapState?.flashElement(elementId);
        ctx.widgetManager.invalidateAll();
    });

    handlers.set(Opcodes.WORLDMAP_FLASHELEMENTCATEGORY, (ctx) => {
        const categoryId = ctx.popInt();
        ctx.worldMapState?.flashCategory(categoryId);
        ctx.widgetManager.invalidateAll();
    });

    handlers.set(Opcodes.WORLDMAP_STOPCURRENTFLASHES, (ctx) => {
        ctx.worldMapState?.stopCurrentFlashes();
        ctx.widgetManager.invalidateAll();
    });

    handlers.set(Opcodes.WORLDMAP_DISABLEELEMENTS, (ctx) => {
        if (ctx.worldMapState) ctx.worldMapState.elementsEnabled = ctx.popInt() === 1;
        else ctx.intStackSize--;
        ctx.widgetManager.invalidateAll();
    });

    handlers.set(Opcodes.WORLDMAP_DISABLEELEMENT, (ctx) => {
        const enabled = ctx.popInt() === 1;
        const elementId = ctx.popInt();
        ctx.worldMapState?.setElementEnabled(elementId, enabled);
        ctx.widgetManager.invalidateAll();
    });

    handlers.set(Opcodes.WORLDMAP_DISABLEELEMENTCATEGORY, (ctx) => {
        const enabled = ctx.popInt() === 1;
        const categoryId = ctx.popInt();
        ctx.worldMapState?.setCategoryEnabled(categoryId, enabled);
        ctx.widgetManager.invalidateAll();
    });

    handlers.set(Opcodes.WORLDMAP_GETDISABLEELEMENTS, (ctx) => {
        ctx.pushInt(ctx.worldMapState?.elementsEnabled ? 1 : 0);
    });

    handlers.set(Opcodes.WORLDMAP_GETDISABLEELEMENT, (ctx) => {
        const elementId = ctx.popInt();
        ctx.pushInt(ctx.worldMapState?.isElementEnabled(elementId) ? 1 : 0);
    });

    handlers.set(Opcodes.WORLDMAP_GETDISABLEELEMENTCATEGORY, (ctx) => {
        const categoryId = ctx.popInt();
        ctx.pushInt(ctx.worldMapState?.isCategoryEnabled(categoryId) ? 1 : 0);
    });

    handlers.set(Opcodes.WORLDMAP_LISTELEMENT_START, (ctx) => {
        const icon = ctx.worldMapState?.iconStart();
        ctx.pushInt(icon?.element ?? -1);
        ctx.pushInt(icon?.coord ?? -1);
    });

    handlers.set(Opcodes.WORLDMAP_LISTELEMENT_NEXT, (ctx) => {
        const icon = ctx.worldMapState?.iconNext();
        ctx.pushInt(icon?.element ?? -1);
        ctx.pushInt(icon?.coord ?? -1);
    });

    handlers.set(Opcodes.WORLDMAP_ELEMENT, (ctx) => {
        ctx.pushInt(ctx.worldMapState?.currentEvent?.element ?? 0);
    });

    handlers.set(Opcodes.WORLDMAP_ELEMENTCOORD1, (ctx) => {
        ctx.pushInt(ctx.worldMapState?.currentEvent?.coord1 ?? 0);
    });

    handlers.set(Opcodes.WORLDMAP_ELEMENTCOORD, (ctx) => {
        ctx.pushInt(ctx.worldMapState?.currentEvent?.coord2 ?? 0);
    });
}
