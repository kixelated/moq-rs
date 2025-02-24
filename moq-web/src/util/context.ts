export class Context {
	done: Promise<never>;
	cancel!: (reason: unknown) => void;

	constructor() {
		this.done = new Promise((_, reject) => {
			this.cancel = reject;
		});
	}
}
