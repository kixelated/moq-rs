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
export function attribute<T extends AttributeType>(
	value: {
		get: () => T;
		set: (value: T) => void;
	},
	context: ClassAccessorDecoratorContext,
) {
	const name = String(context.name);
	let init: T;

	return {
		init(value: T): T {
			init = value;
			return value;
		},
		get(this: HTMLElement): T {
			const value = this.getAttribute(name);
			return stringToAttribute(value, init);
		},
		set(this: HTMLElement, newValue: T) {
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
	if (typeof value === "string") {
		return value;
	}
	if (typeof value === "number") {
		return value.toString();
	}
	if (typeof value === "boolean") {
		return "";
	}
	throw new Error("Unsupported attribute type");
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
