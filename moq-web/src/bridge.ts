import * as Comlink from "comlink";

import { Watch, Publish, PublishVideo } from "../../pkg";
export { Watch, Publish, PublishVideo };

export class Bridge {
	watch(src: string): Watch & Comlink.ProxyMarked {
		return Comlink.proxy(new Watch(src));
	}

	async publish(src: string): Promise<Publish & Comlink.ProxyMarked> {
		return Comlink.proxy(await Publish.connect(src));
	}
}

// Signal that we're done loading the WASM module.
postMessage("loaded");

// Technically, there's a race condition here...
Comlink.expose(new Bridge());
