import * as Comlink from "comlink";

import * as Rust from "../pkg/moq_web";
export * from "../pkg/moq_web";

export class Api {
	async watch(
		server: string,
		room: string,
		broadcast: string,
	): Promise<Rust.Watch & Comlink.ProxyMarked> {
		return Comlink.proxy(new Rust.Watch(server, room, broadcast));
	}
}

// Signal that we're done loading the WASM module.
postMessage("loaded");

// Technically, there's a race condition here...
Comlink.expose(new Api());
