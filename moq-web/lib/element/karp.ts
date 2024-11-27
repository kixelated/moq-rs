import type * as Moq from "..";

export class MoqKarp extends HTMLElement {
	static get observedAttributes() {
		return ["addr", "broadcast", "action", "muted", "blinded"];
	}

	#addr: string | null = null;
	#room: string | null = null;
	#name: string | null = null;
	#action?: "publish" | "watch" | null = null;
	#muted = false;
	#blinded = false;

	setAttr(name: string, oldValue: string | null, newValue: string | null) {}

	connectedCallback() {
		for (const name of MoqKarp.observedAttributes) {
			const value = this.getAttribute(name);
			if (value !== null) {
				this.attributeChangedCallback(name, null, this.getAttribute(name));
			}
		}
	}

	disconnectedCallback() {
		console.log("Custom element removed from page.");
	}

	adoptedCallback() {
		console.log("Custom element moved to new page.");
	}

	attributeChangedCallback(
		name: string,
		_old: string | null,
		value: string | null,
	) {
		switch (name) {
			case "addr":
				this.#addr = value;
				break;
			case "room":
				this.#room = value;
				break;
			case "name":
				this.#name = value;
				break;
			case "action":
				if (value === "publish" || value === "watch" || value === null) {
					this.#action = value;
				} else {
					throw new Error(`Invalid value for action: ${value}`);
				}
				break;
			case "muted":
				this.#muted = value !== null;
				break;
			case "blinded":
				this.#blinded = value !== null;
				break;
		}
	}
}

customElements.define("moq-karp", MoqKarp);
