export function decode(str: string): Uint8Array {
	const bytes = new Uint8Array(str.length / 2);
	for (let i = 0; i < bytes.length; i += 1) {
		bytes[i] = Number.parseInt(str.slice(2 * i, 2 * i + 2), 16);
	}
	return bytes;
}

export function encode(bytes: Uint8Array): string {
	return Array.from(bytes)
		.map((byte) => byte.toString(16).padStart(2, "0"))
		.join("");
}
