import * as Rust from "@rust";
import { type ConnectionStatus, convertConnectionStatus } from "../connection";

/// Live is fired once when all broadcasts have been discovered (on startup).
export type MeetAction = "join" | "leave" | "live";

export class Meet {
	#inner: Rust.Meet;

	constructor() {
		this.#inner = new Rust.Meet();
	}

	get url(): string | null {
		return this.#inner.url ?? null;
	}

	set url(value: string | null) {
		this.#inner.url = value;
	}

	async *connectionStatus(): AsyncGenerator<ConnectionStatus> {
		const status = this.#inner.status();
		for (;;) {
			const next = await status.connection();
			yield convertConnectionStatus(next);
		}
	}

	async *members(): AsyncGenerator<[MeetAction, string]> {
		const announced = this.#inner.announced();
		while (true) {
			const announce = await announced.next();
			if (announce === undefined) {
				return;
			}

			switch (announce.action) {
				case Rust.MeetAction.Join:
					yield ["join", announce.name];
					break;
				case Rust.MeetAction.Leave:
					yield ["leave", announce.name];
					break;
				case Rust.MeetAction.Live:
					yield ["live", ""];
					break;
				default: {
					const _exhaustive: never = announce.action;
					throw new Error(_exhaustive);
				}
			}
		}
	}
}

export default Meet;
