export type Listener = { cancel(): void };

export class Events<Events extends Record<string, unknown>> {
	#target = new EventTarget();

	on<K extends keyof Events>(
		type: K,
		listener: Events[K] extends undefined ? () => void : (value: Events[K]) => void,
	): Listener {
		const wrapper = (e: Event) => {
			listener((e as CustomEvent<Events[K]>).detail);
		};

		this.#target.addEventListener(type as string, wrapper);

		return {
			cancel: () => {
				this.#target.removeEventListener(type as string, wrapper);
			},
		};
	}

	emit<K extends keyof Events>(type: K, ...value: Events[K] extends undefined ? [] : [Events[K]]) {
		this.#target.dispatchEvent(
			new CustomEvent<Events[K]>(type as string, {
				detail: value[0],
			}),
		);
	}
}
