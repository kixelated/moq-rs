// I hate javascript
export function asError(e: unknown): Error {
	if (e instanceof Error) {
		return e;
	}
	if (typeof e === "string") {
		return new Error(e);
	}
	return new Error(String(e));
}

export function isError(e: unknown): e is Error {
	return e instanceof Error;
}

export class Closed extends Error {
	readonly code: number | null;

	constructor(code?: number) {
		super(`closed code=${code}`);
		this.code = code ?? null;
	}

	static from(err: unknown): Closed {
		return new Closed(Closed.extract(err));
	}

	static extract(err: unknown): number {
		if (err instanceof WebTransportError && err.streamErrorCode !== null) {
			return err.streamErrorCode;
		}

		return 0;
	}
}
