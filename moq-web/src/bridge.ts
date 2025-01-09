import * as Comlink from "comlink";
import type { Api } from "./worker";

// Create a new worker instance
// We wait until the worker is fully initialized before we return the proxy.
function init(): Promise<Comlink.Remote<Api>> {
	return new Promise((resolve) => {
		const worker = new Worker(new URL("./worker", import.meta.url), { type: "module" });
		worker.addEventListener(
			"message",
			(event) => {
				const proxy: Comlink.Remote<Api> = Comlink.wrap(worker);
				resolve(proxy);
			},
			{ once: true },
		);
	});
}

export { init };
