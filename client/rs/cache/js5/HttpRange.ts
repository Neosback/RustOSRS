export type HttpByteRange = {
    start: number;
    endExclusive: number;
    total?: number;
};

const CONTENT_RANGE_PATTERN = /^bytes\s+(\d+)-(\d+)\/(\d+|\*)$/i;

/** Parse and sanity-check an HTTP `Content-Range` header. */
export function parseContentRange(value: string | null): HttpByteRange | undefined {
    const match = value?.trim().match(CONTENT_RANGE_PATTERN);
    if (!match) {
        return undefined;
    }

    const start = Number(match[1]);
    const endInclusive = Number(match[2]);
    const total = match[3] === "*" ? undefined : Number(match[3]);
    if (
        !Number.isSafeInteger(start) ||
        !Number.isSafeInteger(endInclusive) ||
        start < 0 ||
        endInclusive < start ||
        (total !== undefined &&
            (!Number.isSafeInteger(total) || total <= 0 || endInclusive >= total))
    ) {
        return undefined;
    }

    return { start, endExclusive: endInclusive + 1, total };
}

/**
 * Require a response to describe exactly the byte interval that was requested.
 * A `206` with a missing or mismatched `Content-Range` is not safe to copy into
 * the sparse dat2 buffer because it could mark unrelated sectors as present.
 */
export function validatePartialContentResponse(
    response: Response,
    expectedStart: number,
    expectedEndExclusive: number,
    resource: string,
): HttpByteRange {
    if (response.status !== 206) {
        throw new Error(`Expected HTTP 206 for ${resource}, received ${response.status}`);
    }
    const range = parseContentRange(response.headers.get("Content-Range"));
    if (!range) {
        throw new Error(`Invalid Content-Range for ${resource}`);
    }
    if (range.start !== expectedStart || range.endExclusive !== expectedEndExclusive) {
        throw new Error(
            `Unexpected Content-Range for ${resource}: requested ${expectedStart}-${expectedEndExclusive - 1}, ` +
                `received ${range.start}-${range.endExclusive - 1}`,
        );
    }
    return range;
}
