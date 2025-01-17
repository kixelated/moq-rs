declare namespace JSX {
	type Element = HTMLElementTagNameMap[keyof HTMLElementTagNameMap];

	type IntrinsicElements = {
		[K in keyof HTMLElementTagNameMap]: Partial<HTMLElementTagNameMap[K]> & {
			children?: (Node | string)[];
			ref?: (el: HTMLElementTagNameMap[T]) => void;
			css?: Partial<CSSStyleDeclaration>;
		};
	};
}
