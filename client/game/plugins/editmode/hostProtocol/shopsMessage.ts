export const SHOP_DEFINITIONS_MESSAGE = "elvarg:shop-definitions";
export const SHOP_DEFINITIONS_REQUEST_MESSAGE = "elvarg:shop-definitions-request";

export type ShopDefinitionsMessage = { type: typeof SHOP_DEFINITIONS_MESSAGE; contents: string };
