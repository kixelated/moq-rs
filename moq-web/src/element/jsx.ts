/// <reference path="./jsx.d.ts" />

type JSXProps<T extends keyof HTMLElementTagNameMap> = Partial<HTMLElementTagNameMap[T]> & {
	children?: (Node | string)[];
	ref?: (el: HTMLElementTagNameMap[T]) => void;
	css?: Partial<CSSStyleDeclaration>;
};

export function jsx(tag: typeof jsxFragment, props: null, ...children: (Node | string)[]): DocumentFragment;
export function jsx<T extends keyof HTMLElementTagNameMap>(
	tag: T,
	props: JSXProps<T>,
	...children: (Node | string)[]
): HTMLElementTagNameMap[T];

// biome-ignore lint/suspicious/noExplicitAny: overloaded function
export function jsx(tag: any, props: any, ...children: any[]): HTMLElement | DocumentFragment {
	if (tag === jsxFragment) {
		return jsxFragment(...children);
	}

	const element = document.createElement(tag as keyof HTMLElementTagNameMap);

	if (props) {
		for (const [key, value] of Object.entries(props)) {
			if (key === "children") continue;

			if (key === "ref" && typeof value === "function") {
				value(element);
			} else if (key === "css" && typeof value === "object") {
				Object.assign(element.style, value);
			} else if (key.startsWith("on") && typeof value === "function") {
				const eventName = key.slice(2).toLowerCase(); // e.g., onClick -> click
				element.addEventListener(eventName, value as EventListener);
			} else if (key in element) {
				Reflect.set(element, key, value); // Set as property
			} else {
				element.setAttribute(key, value as string); // Fallback to attribute
			}
		}
	}

	for (const child of children) {
		if (typeof child === "string") {
			element.appendChild(document.createTextNode(child));
		} else {
			element.appendChild(child);
		}
	}

	return element;
}

export function jsxFragment(...children: (Node | string)[]): DocumentFragment {
	const fragment = document.createDocumentFragment();
	for (const child of children) {
		if (typeof child === "string") {
			fragment.appendChild(document.createTextNode(child));
		} else {
			fragment.appendChild(child);
		}
	}
	return fragment;
}
