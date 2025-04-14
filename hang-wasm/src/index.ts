import type { Command, Event } from "./rpc";
export type { Command, Event };

export class Backend {
	#worker: Promise<Worker>;

	constructor() {
		this.#worker = new Promise((resolve, reject) => {
			const worker = new Worker(new URL("../pkg", import.meta.url), {
				type: "module",
			});

			worker.addEventListener(
				"message",
				(event: MessageEvent<Event>) => {
					if (event.data.event === "Init") {
						resolve(worker);
					} else {
						reject(new Error(`Unknown init event: ${event.data.event}`));
					}
				},
				{ once: true },
			);
		});
	}

	addEventListener(callback: (event: Event) => void) {
		this.#worker.then((worker) => {
			worker.addEventListener("message", (event: MessageEvent<Event>) => {
				callback(event.data);
			});
		});
	}

	postMessage(msg: Command) {
		const transfer: Transferable[] = [];
		if (msg.command === "Canvas") {
			transfer.push(msg.canvas);
		}

		this.#worker.then((worker) => {
			worker.postMessage(msg, { transfer });
		});
	}
}

export default Backend;
