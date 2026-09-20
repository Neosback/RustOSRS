export type CustomModelRow = { id: number; data: string };

// Keep encoded bytes: callers recolour/resize decoded ModelData in place.
const models = new Map<number, { row: CustomModelRow; bytes: Int8Array }>();

export const CustomModelRegistry = {
    revision: 0,
    register(row: CustomModelRow): void {
        if (!Number.isInteger(row.id) || row.id < 1000000 || row.id > 0x7fffffff ||
            typeof row.data !== "string" || row.data.length > 1400000) {
            throw new Error("Invalid custom model");
        }
        const decoded = atob(row.data);
        if (decoded.length < 18) throw new Error("Custom model is truncated");
        models.set(row.id, { row, bytes: Int8Array.from(decoded, (byte) => byte.charCodeAt(0)) });
        this.revision++;
    },
    get(id: number): Int8Array | undefined { return models.get(id)?.bytes; },
    getAll(): CustomModelRow[] { return Array.from(models.values(), ({ row }) => row); },
    clear(): void { models.clear(); this.revision++; },
};
