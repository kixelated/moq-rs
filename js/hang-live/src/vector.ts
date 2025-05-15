export class Vector {
	x: number;
	y: number;

	constructor(x: number, y: number) {
		this.x = x;
		this.y = y;
	}

	static create(x: number, y: number) {
		return new Vector(x, y);
	}

	mult(scalar: number) {
		return new Vector(this.x * scalar, this.y * scalar);
	}

	normalize() {
		const length = this.length();
		return new Vector(this.x / length, this.y / length);
	}

	add(other: Vector) {
		return new Vector(this.x + other.x, this.y + other.y);
	}

	sub(other: Vector) {
		return new Vector(this.x - other.x, this.y - other.y);
	}

	div(scalar: number) {
		return new Vector(this.x / scalar, this.y / scalar);
	}

	length() {
		return Math.sqrt(this.x * this.x + this.y * this.y);
	}

	clone() {
		return new Vector(this.x, this.y);
	}
}
