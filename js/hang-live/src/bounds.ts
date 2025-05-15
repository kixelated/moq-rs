import { Vector } from "./vector";

export class Bounds {
	position: Vector;
	size: Vector;

	constructor(position: Vector, size: Vector) {
		this.position = position;
		this.size = size;
	}

	static dom(el: DOMRect) {
		return new Bounds(Vector.create(el.x, el.y), Vector.create(el.width, el.height));
	}

	middle() {
		return Vector.create(this.position.x + this.size.x / 2, this.position.y + this.size.y / 2);
	}

	area() {
		return this.size.x * this.size.y;
	}

	add(v: Vector) {
		return new Bounds(this.position.add(v), this.size);
	}

	sub(v: Vector) {
		return new Bounds(this.position.sub(v), this.size);
	}

	mult(v: number) {
		return new Bounds(this.position, this.size.mult(v));
	}

	div(v: number) {
		return new Bounds(this.position, this.size.div(v));
	}

	intersects(b: Bounds) {
		// Compute the intersection rectangle.
		const left = Math.max(this.position.x, b.position.x);
		const right = Math.min(this.position.x + this.size.x, b.position.x + b.size.x);
		const top = Math.max(this.position.y, b.position.y);
		const bottom = Math.min(this.position.y + this.size.y, b.position.y + b.size.y);

		if (left >= right || top >= bottom) {
			return;
		}

		return new Bounds(Vector.create(left, top), Vector.create(right - left, bottom - top));
	}

	contains(p: Vector): boolean {
		return (
			p.x >= this.position.x &&
			p.x <= this.position.x + this.size.x &&
			p.y >= this.position.y &&
			p.y <= this.position.y + this.size.y
		);
	}
}
