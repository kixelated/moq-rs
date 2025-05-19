import { createSignal } from "solid-js";
import { render } from "solid-js/web";
import { Support, SupportRole, SupportShow } from "./";

export class SupportElement extends HTMLElement {
	#role = createSignal<SupportRole>("all");
	#show = createSignal<SupportShow>("full");

	static get observedAttributes() {
		return ["role", "show"];
	}

	attributeChangedCallback(name: string, _oldValue?: string, newValue?: string) {
		if (name === "role") {
			const role = newValue ?? "all";

			if (role === "core" || role === "watch" || role === "publish" || role === "all") {
				this.#role[1](role);
			} else {
				throw new Error(`Invalid role: ${role}`);
			}
		} else if (name === "show") {
			const show = newValue ?? "full";
			if (show === "full" || show === "partial" || show === "none") {
				this.#show[1](show);
			} else {
				throw new Error(`Invalid show: ${show}`);
			}
		}
	}

	connectedCallback() {
		const root = this.appendChild(document.createElement("div"));
		render(() => <Support role={this.#role[0]()} show={this.#show[0]()} />, root);
	}
}

customElements.define("hang-support", SupportElement);
