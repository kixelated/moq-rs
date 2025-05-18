import { render } from "solid-js/web";
import { Support } from "./";
import { createSignal } from "solid-js";

export class SupportElement extends HTMLElement {
	#role = createSignal<"watch" | "publish" | "both">("both");

	static get observedAttributes() {
		return ["role"];
	}

	attributeChangedCallback(name: string, _oldValue?: string, newValue?: string) {
		if (name === "role") {
			const role = newValue ?? "both";

			if (role === "watch" || role === "publish" || role === "both") {
				this.#role[1](role);
			} else {
				throw new Error(`Invalid role: ${role}`);
			}
		}
	}

	connectedCallback() {
		const root = this.appendChild(document.createElement("div"));
		render(() => <Support role={this.#role[0]()} />, root);
	}
}

customElements.define("hang-support", SupportElement);
