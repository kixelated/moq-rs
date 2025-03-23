import * as Comlink from "comlink";

import * as Rust from "@rust";
export type { Watch } from "@rust";

export class Bridge {
	async watch(): Promise<Rust.Watch & Comlink.ProxyMarked> {
		// Proxy the Watch instance
		const watch = Comlink.proxy(new Rust.Watch());

		// Wrap the status() method to proxy its result
		const status = watch.status.bind(watch);
		watch.status = Comlink.proxy(() => Comlink.proxy(status()));

		return watch;
	}
}

// Signal that we're done loading the WASM module.
postMessage("loaded");

// Technically, there's a race condition here...
Comlink.expose(new Bridge());
