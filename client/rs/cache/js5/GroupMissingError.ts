export class GroupMissingError extends Error {
    constructor(
        readonly indexId: number,
        readonly archiveId: number,
        readonly startByte: number,
        readonly byteLength: number,
    ) {
        super(`Cache group not downloaded yet: index=${indexId} archive=${archiveId}`);
        this.name = "GroupMissingError";
    }
}

export function isGroupMissingError(e: unknown): e is GroupMissingError {
    return e instanceof GroupMissingError;
}
