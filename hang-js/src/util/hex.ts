export function decode(str: string): Uint8Array {
	const bytes = new Uint8Array(str.length / 2);
	for (let i = 0; i < bytes.length; i += 1) {
		bytes[i] = Number.parseInt(str.slice(2 * i, 2 * i + 2), 16);
	}
	return bytes;
}

export function encode(input: AllowSharedBufferSource): string {
	let bytes: Uint8Array;

	if (input instanceof Uint8Array) {
		bytes = input;
	} else if (ArrayBuffer.isView(input)) {
		bytes = new Uint8Array(input.buffer, input.byteOffset, input.byteLength);
	} else {
		bytes = new Uint8Array(input);
	}

	const hex = new Array(bytes.length);
	for (let i = 0; i < bytes.length; i++) {
		hex[i] = bytes[i].toString(16).padStart(2, "0");
	}

	return hex.join("");
}
