/**
 * Stock market and trading post operations
 */
import { Opcodes } from "../Opcodes";
import type { HandlerContext, HandlerMap } from "./HandlerTypes";

export function registerMarketOps(handlers: HandlerMap): void {
    const state = (ctx: HandlerContext, slot: number, field: number) =>
        ctx.varManager.getVarp(7900 + slot * 7 + field);
    const slot = (ctx: HandlerContext) => ctx.popInt();

    // === Stock Market ===
    handlers.set(Opcodes.STOCKMARKET_GETOFFERTYPE, (ctx) => {
        const index = slot(ctx);
        ctx.pushInt(state(ctx, index, 4));
    });

    handlers.set(Opcodes.STOCKMARKET_GETOFFERITEM, (ctx) => {
        const index = slot(ctx);
        ctx.pushInt(state(ctx, index, 6));
    });

    handlers.set(Opcodes.STOCKMARKET_GETOFFERPRICE, (ctx) => {
        const index = slot(ctx);
        ctx.pushInt(state(ctx, index, 0));
    });

    handlers.set(Opcodes.STOCKMARKET_GETOFFERCOUNT, (ctx) => {
        const index = slot(ctx);
        ctx.pushInt(state(ctx, index, 1));
    });

    handlers.set(Opcodes.STOCKMARKET_GETOFFERCOMPLETEDCOUNT, (ctx) => {
        const index = slot(ctx);
        ctx.pushInt(state(ctx, index, 2));
    });

    handlers.set(Opcodes.STOCKMARKET_GETOFFERCOMPLETEDGOLD, (ctx) => {
        const index = slot(ctx);
        ctx.pushInt(state(ctx, index, 3));
    });

    handlers.set(Opcodes.STOCKMARKET_ISOFFEREMPTY, (ctx) => {
        const index = slot(ctx);
        ctx.pushInt(state(ctx, index, 5) === 0 ? 1 : 0);
    });

    handlers.set(Opcodes.STOCKMARKET_ISOFFERSTABLE, (ctx) => {
        const index = slot(ctx);
        ctx.pushInt(state(ctx, index, 5) === 2 ? 1 : 0);
    });

    handlers.set(Opcodes.STOCKMARKET_ISOFFERFINISHED, (ctx) => {
        const index = slot(ctx);
        ctx.pushInt(state(ctx, index, 5) === 5 ? 1 : 0);
    });

    handlers.set(Opcodes.STOCKMARKET_ISOFFERADDING, (ctx) => {
        const index = slot(ctx);
        ctx.pushInt(state(ctx, index, 5) === 1 ? 1 : 0);
    });

    // === Trading Post ===
    handlers.set(Opcodes.TRADINGPOST_SORTBY_NAME, (ctx) => {
        ctx.intStackSize--; // pop ascending
    });

    handlers.set(Opcodes.TRADINGPOST_SORTBY_PRICE, (ctx) => {
        ctx.intStackSize--; // pop ascending
    });

    handlers.set(Opcodes.TRADINGPOST_SORTFILTERBY_WORLD, (ctx) => {
        ctx.intStackSize -= 2; // pop world, ascending
    });

    handlers.set(Opcodes.TRADINGPOST_SORTBY_AGE, (ctx) => {
        ctx.intStackSize--; // pop ascending
    });

    handlers.set(Opcodes.TRADINGPOST_SORTBY_COUNT, (ctx) => {
        ctx.intStackSize--; // pop ascending
    });

    handlers.set(Opcodes.TRADINGPOST_GETTOTALOFFERS, (ctx) => {
        ctx.pushInt(0);
    });

    handlers.set(Opcodes.TRADINGPOST_GETOFFERWORLD, (ctx) => {
        ctx.intStackSize--; // pop index
        ctx.pushInt(0);
    });

    handlers.set(Opcodes.TRADINGPOST_GETOFFERNAME, (ctx) => {
        ctx.intStackSize--; // pop index
        ctx.pushString("");
    });

    handlers.set(Opcodes.TRADINGPOST_GETOFFERPREVIOUSNAME, (ctx) => {
        ctx.intStackSize--; // pop index
        ctx.pushString("");
    });

    handlers.set(Opcodes.TRADINGPOST_GETOFFERAGE, (ctx) => {
        ctx.intStackSize--; // pop index
        ctx.pushInt(0);
    });

    handlers.set(Opcodes.TRADINGPOST_GETOFFERCOUNT, (ctx) => {
        ctx.intStackSize--; // pop index
        ctx.pushInt(0);
    });

    handlers.set(Opcodes.TRADINGPOST_GETOFFERPRICE, (ctx) => {
        ctx.intStackSize--; // pop index
        ctx.pushInt(0);
    });

    handlers.set(Opcodes.TRADINGPOST_GETOFFERITEM, (ctx) => {
        ctx.intStackSize--; // pop index
        ctx.pushInt(-1);
    });

    // === Hiscores ===
    handlers.set(Opcodes.HISCORE_GETSTATUS, (ctx) => {
        // Returns hiscores fetch status:
        // 0 = not fetched, 1 = loading, 2 = loaded
        // For now, return 0 (not fetched) as hiscores aren't implemented
        ctx.pushInt(0);
    });
}
