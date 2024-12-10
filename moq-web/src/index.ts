// Export the library
export { Watch } from "./watch";
export { Publish } from "./publish";

import * as Comlink from "comlink";
import type { Bridge } from "./bridge";

const worker = new Worker(new URL("./bridge", import.meta.url), { type: "module" });

// Not 100% if this is the best solution, but Comlink was silently failing.
// We wait until the worker is fully initialized before we return the proxy.
const init: Promise<Comlink.Remote<Bridge>> = new Promise((resolve) => {
	worker.onmessage = (_event) => {
		worker.onmessage = null;
		const proxy: Comlink.Remote<Bridge> = Comlink.wrap(worker);
		resolve(proxy);
	};
});

export { init };
