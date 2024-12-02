import * as Comlink from "comlink";
import type { Api } from "./worker";

const worker = new Worker(new URL("./worker", import.meta.url), { type: "module" });

// Not 100% if this is the best solution, but Comlink was silently failing.
// We wait until the worker is fully initialized before we return the proxy.
const init: Promise<Comlink.Remote<Api>> = new Promise((resolve) => {
	worker.onmessage = (_event) => {
		worker.onmessage = null;
		const proxy: Comlink.Remote<Api> = Comlink.wrap(worker);
		resolve(proxy);
	};
});

export { init };
