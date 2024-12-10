import * as Comlink from "comlink";

import { Watch, Publish } from "../../pkg";
export { Watch, Publish };

export class Bridge {
	async watch(src: string): Promise<Watch & Comlink.ProxyMarked> {
		return Comlink.proxy(new Watch(src));
	}

	async publish(src: string): Promise<Publish & Comlink.ProxyMarked> {
		return Comlink.proxy(new Publish(src));
	}
}

// Signal that we're done loading the WASM module.
postMessage("loaded");

// Technically, there's a race condition here...
Comlink.expose(new Bridge());
