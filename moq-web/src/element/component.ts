const attributesMetadataKey = Symbol("attributes");

// Ensure metadata is enabled. TypeScript does not polyfill
// Symbol.metadata, so we must ensure that it exists.
(Symbol as { metadata: symbol }).metadata ??= Symbol("metadata");

interface MoqClassDecoratorTarget {
	new (): MoqElement;
}

export function element(name: string) {
	return (construct: MoqClassDecoratorTarget, context: ClassDecoratorContext) => {
		context.addInitializer(() => {
			customElements.define(name, construct);
		});
	};
}

export class MoqElement extends HTMLElement {
	static get observedAttributes(): string[] {
		// biome-ignore lint/complexity/noThisInStatic: Required for inheritance
		const metadata = this[Symbol.metadata];
		if (!metadata) {
			return [];
		}

		const attributes = metadata[attributesMetadataKey] as Map<string, AttributeType> | undefined;
		if (!attributes) {
			return [];
		}

		return Array.from(attributes.keys());
	}

	attributeChangedCallback(name: string, old: string | null, value: string | null) {
		if (old === value) {
			return;
		}

		// Convert the attribute name to camelCase and kebab-case.
		const camel = name.replace(/_([a-z])/g, (g) => g[1].toUpperCase());
		const handler = `${camel}Change` as const;

		// biome-ignore lint/suspicious/noExplicitAny: Accessor must exist
		const typed = (this as any)[name];

		// Fire a custom events indicating an attribute has changed.
		this.dispatchEvent(new MoqAttrEvent({ name, value: typed }));

		// biome-ignore lint/suspicious/noExplicitAny: Look for optional `xxxChange` method
		const f = (this as any)[handler];
		if (typeof f === "function") {
			f.bind(this)(typed);
		}
	}
}

export type AttributeType = string | number | boolean;

// Pretty proud of this one.
// This is a decorator that modifies an accessor to read/write an attribute on an HTMLElement.
//
// The default value of the accessor is used determine the zero value.
// This is important, otherwise we don't know how to parse a string into the correct type.
// We use this value to determine when the attribute should be present or removed.
// This works since the default element has no attributes set.
//
// ex.
// @attribute
// accessor muted = false;
//
// We use the HTMLElement.getAttribute method to get the value of the attribute.
// In this case, if <element muted> then get() returns true.
// If we then set `element.muted=false`, it will remove the attribute.
export function attribute<C extends Element, V extends AttributeType>(
	target: ClassAccessorDecoratorTarget<C, V>,
	context: ClassAccessorDecoratorContext<C, V>,
): ClassAccessorDecoratorResult<C, V> {
	const name = String(context.name);

	// biome-ignore lint/suspicious/noAssignInExpressions: Simpler than multiple lines
	const attributes = ((context.metadata[attributesMetadataKey] as Set<string> | undefined) ??= new Set());
	attributes.add(name);

	let init: V;

	return {
		init(value: V): V {
			init = value;
			return value;
		},
		get(this: C): V {
			const value = this.getAttribute(name);
			return stringToAttribute(value, init);
		},
		set(this: C, newValue: V) {
			if (newValue === init) {
				this.removeAttribute(name);
			} else {
				const str = attributeToString(newValue);
				this.setAttribute(name, str);
			}
		},
	};
}

function attributeToString(value: AttributeType): string {
	switch (typeof value) {
		case "string":
			return value;
		case "number":
			return value.toString();
		case "boolean":
			return "";
	}
}

function stringToAttribute<T extends AttributeType>(value: string | null, init: T): T {
	if (value === null) {
		return init;
	}

	switch (typeof init) {
		case "string":
			return value as T;
		case "number":
			return Number.parseFloat(value) as T;
		case "boolean":
			return !init as T;
		default:
			throw new Error("Unsupported attribute type");
	}
}

interface MoqAttrEventDetail<T extends AttributeType> {
	name: string;
	value: T;
}

class MoqAttrEvent<T extends AttributeType = AttributeType> extends CustomEvent<MoqAttrEventDetail<T>> {
	constructor(detail: MoqAttrEventDetail<T>) {
		super("moq-attr", { detail, bubbles: true, composed: true });
	}
}

declare global {
	interface HTMLElementEventMap {
		"moq-attr": MoqAttrEvent;
	}
}
