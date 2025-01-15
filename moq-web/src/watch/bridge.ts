import * as Comlink from "comlink";

import * as Rust from "../../../pkg";
export type { Watch } from "../../../pkg";

export class Bridge {
	async watch(): Promise<Rust.Watch & Comlink.ProxyMarked> {
		// Proxy the Watch instance
		const watch = Comlink.proxy(new Rust.Watch());

		// Wrap the states() method to proxy its result
		const states = watch.states.bind(watch);
		watch.states = Comlink.proxy(() => Comlink.proxy(states()));

		return watch;
	}
}

// Signal that we're done loading the WASM module.
postMessage("loaded");

// Technically, there's a race condition here...
Comlink.expose(new Bridge());
