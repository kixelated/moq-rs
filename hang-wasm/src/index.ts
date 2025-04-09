import type { Command } from "./command";
export type { Command };

const worker = new Worker(new URL("../pkg", import.meta.url), {
	type: "module",
});

export function command(command: Command) {
	worker.postMessage(command);
}
