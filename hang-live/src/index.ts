import Matter from "matter-js";

const PADDING = 40;

const NORMAL_CATEGORY = 0x0001;
const DRAG_CATEGORY = 0x0002;

// Canvas setup
const canvas = document.getElementById("canvas") as HTMLCanvasElement;
function resizeCanvas() {
	for (const box of boxes) {
		box.targetX *= window.innerWidth / canvas.width;
		box.targetY *= window.innerHeight / canvas.height;
	}
	canvas.width = window.innerWidth;
	canvas.height = window.innerHeight;
	updateScale();
}

window.addEventListener("resize", resizeCanvas);

const ctx = canvas.getContext("2d");
if (!ctx) {
	throw new Error("Failed to get canvas context");
}
ctx.font = "12px sans-serif";

// PARTICIPANT STRUCTURE
interface Box {
	id: string;
	w: number;
	h: number;
	targetX: number;
	targetY: number;
	sourceW: number;
	sourceH: number;
	targetScale: number;
	scale: number;
	body: Matter.Body;
}

// SETUP PHYSICS ENGINE (no gravity)
const engine = Matter.Engine.create();
engine.gravity.x = 0;
engine.gravity.y = 0;
//engine.positionIterations = 1;
//engine.velocityIterations = 1;

const randomStart = () => {
	// Cluster around 0.5
	return 0.5 + (Math.random() - 0.5) * Math.random();
};

const boxes: Box[] = [];

function findVacantPoint(): { x: number; y: number } {
	const NUM_CANDIDATES = 50;

	for (let i = 0; i < NUM_CANDIDATES; i++) {
		const x = randomStart() * canvas.width;
		const y = randomStart() * canvas.height;

		// Query the world for overlapping bodies
		const overlapping = Matter.Query.point(engine.world.bodies, { x, y });

		if (overlapping.length === 0) {
			return { x, y };
		}
	}

	return { x: randomStart() * canvas.width, y: randomStart() * canvas.height };
}

function join(info: {
	id: string;
	w: number;
	h: number;
}) {
	const { x, y } = findVacantPoint();

	const cx = canvas.width / 2;
	const cy = canvas.height / 2;

	// Vector from center to target
	const dx = x - cx;
	const dy = y - cy;
	const len = Math.sqrt(dx * dx + dy * dy);

	// Ensure it's fully offscreen: offset by desired margin + box radius
	const boxRadius = Math.sqrt(info.w ** 2 + info.h ** 2);
	const margin = 60;
	const offset = boxRadius + margin;

	const unitX = dx / len;
	const unitY = dy / len;

	const startX = x + unitX * offset;
	const startY = y + unitY * offset;

	const body = Matter.Bodies.rectangle(startX, startY, info.w, info.h, {
		inertia: Number.POSITIVE_INFINITY,
		restitution: 0.1,
		frictionAir: 0.3,
	});

	body.collisionFilter.category = NORMAL_CATEGORY;
	body.collisionFilter.mask = NORMAL_CATEGORY;

	const box = {
		id: info.id,
		w: info.w,
		h: info.h,
		targetX: x,
		targetY: y,
		sourceW: info.w,
		sourceH: info.h,
		targetScale: 1,
		scale: 1,
		body,
	};

	boxes.push(box);
	Matter.World.add(engine.world, body);

	updateScale();

	return box;
}

join({
	id: "A",
	w: 1920,
	h: 1080,
});

setTimeout(() => {
	join({
		id: "B",
		w: 1280,
		h: 720,
	});
}, 1000);

setTimeout(() => {
	join({
		id: "C",
		w: 768,
		h: 1024,
	});
}, 2000);

setTimeout(() => {
	join({
		id: "D",
		w: 360,
		h: 640,
	});
}, 3000);

setTimeout(() => {
	join({
		id: "E",
		w: 1024,
		h: 768,
	});
}, 5000);

let lastTime = performance.now();

// Simulation loop
function tick() {
	updateScale();

	for (const box of boxes) {
		const { scale, targetScale } = box;
		if (Math.abs(targetScale - scale) > 0.001) {
			const newScale = scale + (targetScale - scale) * 0.1;
			box.scale = newScale;
			recreateBody(box);
		}
	}

	for (const box of boxes) {
		const body = box.body;

		const dx = box.targetX - body.position.x;
		const dy = box.targetY - body.position.y;
		let stiffness = 0.001;
		if (dragging && dragging.id === box.id) {
			stiffness *= dragging.body.mass / 10;
		}

		Matter.Body.applyForce(body, body.position, { x: dx * stiffness, y: dy * stiffness });

		// Boundary repulsion
		const left = PADDING + box.w / 2;
		const right = canvas.width - PADDING - box.w / 2;
		const top = PADDING + box.h / 2;
		const bottom = canvas.height - PADDING - box.h / 2;

		const wallStrength = 0.01;
		if (body.position.x < left) {
			Matter.Body.applyForce(body, body.position, { x: (left - body.position.x) * wallStrength, y: 0 });
		} else if (body.position.x > right) {
			Matter.Body.applyForce(body, body.position, { x: (right - body.position.x) * wallStrength, y: 0 });
		}

		if (body.position.y < top) {
			Matter.Body.applyForce(body, body.position, { x: 0, y: (top - body.position.y) * wallStrength });
		} else if (body.position.y > bottom) {
			Matter.Body.applyForce(body, body.position, { x: 0, y: (bottom - body.position.y) * wallStrength });
		}

		// Slowly drift the targetX/Y towards the actual box.
		// (reduces fighting the physics engine)
		box.targetX += (box.body.position.x - box.targetX) * 0.001;
		box.targetY += (box.body.position.y - box.targetY) * 0.001;
	}

	if (dragging) {
		const collisions = Matter.Query.collides(dragging.body, engine.world.bodies);
		for (const collision of collisions) {
			if (collision.bodyA === dragging.body && collision.bodyB === dragging.body) {
				continue;
			}

			const other = collision.bodyA === dragging.body ? collision.bodyB : collision.bodyA;

			const { normal, depth } = collision;
			if (collision.bodyA === dragging.body) {
				normal.x = -normal.x;
				normal.y = -normal.y;
			}

			const strength = 0.001;
			const force = depth * strength;

			Matter.Body.applyForce(other, other.position, {
				x: normal.x * force,
				y: normal.y * force,
			});
		}
	}

	const now = performance.now();

	Matter.Engine.update(engine, now - lastTime);
	lastTime = now;

	render();
	requestAnimationFrame(tick);
}

// Render loop
function render() {
	if (!ctx) return;
	ctx.clearRect(0, 0, canvas.width, canvas.height);

	for (const box of boxes) {
		ctx.save();
		if (box !== dragging) {
			ctx.fillStyle = "#4b9";
			renderBox(ctx, box);
		}
		ctx.restore();
	}

	if (dragging) {
		ctx.save();
		// Apply a transparent overlay
		ctx.fillStyle = "rgba(0, 0, 0, 0.5)";
		renderBox(ctx, dragging);
		ctx.restore();
	}
}

function renderBox(ctx: CanvasRenderingContext2D, box: Box) {
	const { x, y } = box.body.position;
	const angle = box.body.angle;

	ctx.translate(x, y);
	ctx.rotate(angle);
	ctx.fillRect(-box.w / 2, -box.h / 2, box.w, box.h);
	ctx.strokeStyle = "#000";
	ctx.strokeRect(-box.w / 2, -box.h / 2, box.w, box.h);
	ctx.fillStyle = "#000";
	ctx.fillText(box.id, -box.w / 2 + 4, -box.h / 2 + 14);

	// Draw target
	/*
	ctx.beginPath();
	ctx.arc(box.targetX, box.targetY, 4, 0, 2 * Math.PI);
	ctx.fillStyle = "#f00";
	ctx.fill();
	*/
}

function updateScale() {
	const padding = 20;
	const usableW = canvas.width - 2 * padding;
	const usableH = canvas.height - 2 * padding;
	const canvasArea = usableW * usableH;

	const totalBoxArea = boxes.reduce((sum, b) => sum + b.sourceW * b.sourceH * b.targetScale * b.targetScale, 0);

	const fillRatio = totalBoxArea / canvasArea;
	const targetFill = 0.4;

	const scale = Math.sqrt(targetFill / fillRatio);

	// Apply scale to each box
	for (const box of boxes) {
		applyScale(box, scale);
	}
}

resizeCanvas(); // initial call
tick();

let dragging: Box | null = null;

function boxAt(x: number, y: number): Box | null {
	for (const box of boxes) {
		const { x: bx, y: by } = box.body.position;
		const left = bx - box.w / 2;
		const right = bx + box.w / 2;
		const top = by - box.h / 2;
		const bottom = by + box.h / 2;

		if (x >= left && x <= right && y >= top && y <= bottom) {
			return box;
		}
	}

	return null;
}

canvas.addEventListener("mousedown", (e) => {
	const rect = canvas.getBoundingClientRect();
	const mx = e.clientX - rect.left;
	const my = e.clientY - rect.top;

	dragging = boxAt(mx, my);
	if (dragging) {
		dragging.body.collisionFilter.mask = 0x0000;
		canvas.style.cursor = "grabbing";
	}
});

canvas.addEventListener("mousemove", (e) => {
	if (dragging) {
		const rect = canvas.getBoundingClientRect();
		const mx = e.clientX - rect.left;
		const my = e.clientY - rect.top;
		dragging.targetX = mx;
		dragging.targetY = my;
	} else {
		const box = boxAt(e.clientX, e.clientY);
		if (box) {
			canvas.style.cursor = "grab";
		} else {
			canvas.style.cursor = "default";
		}
	}
});

canvas.addEventListener("mouseup", () => {
	if (dragging) {
		dragging.body.collisionFilter.mask = NORMAL_CATEGORY;
		dragging = null;
		canvas.style.cursor = "default";
	}
});

canvas.addEventListener("mouseleave", () => {
	if (dragging) {
		dragging.body.collisionFilter.mask = NORMAL_CATEGORY;
		dragging = null;
		canvas.style.cursor = "default";
	}
});

canvas.addEventListener(
	"wheel",
	(e) => {
		e.preventDefault(); // Prevent scroll

		let box = dragging;
		if (!box) {
			const rect = canvas.getBoundingClientRect();
			const mx = e.clientX - rect.left;
			const my = e.clientY - rect.top;

			for (const b of boxes) {
				const { x, y } = b.body.position;
				const left = x - b.w / 2;
				const right = x + b.w / 2;
				const top = y - b.h / 2;
				const bottom = y + b.h / 2;

				if (mx >= left && mx <= right && my >= top && my <= bottom) {
					box = b;
					break;
				}
			}

			if (!box) return;
		}

		const scale = 1 - e.deltaY * 0.001;
		if (scale < 1) {
			canvas.style.cursor = "zoom-out";
		} else if (scale > 1) {
			canvas.style.cursor = "zoom-in";
		}

		applyScale(box, scale);
	},
	{ passive: false },
);

function applyScale(box: Box, scale: number) {
	const minScale = 0.1;
	const maxScale = Math.min(canvas.width / box.sourceW, canvas.height / box.sourceH, 2);
	box.targetScale = Math.max(Math.min(box.targetScale * scale, maxScale), minScale);
}

function recreateBody(box: Box) {
	const { position, velocity, angle, angularVelocity } = box.body;
	Matter.World.remove(engine.world, box.body);

	const newW = box.sourceW * box.scale;
	const newH = box.sourceH * box.scale;

	const newBody = Matter.Bodies.rectangle(position.x, position.y, newW, newH, {
		inertia: Number.POSITIVE_INFINITY,
		restitution: 0.1,
		frictionAir: 0.3,
	});

	Matter.Body.setVelocity(newBody, velocity);
	Matter.Body.setAngle(newBody, angle);
	Matter.Body.setAngularVelocity(newBody, angularVelocity);
	newBody.collisionFilter.mask = box.body.collisionFilter.mask;

	box.w = newW;
	box.h = newH;
	box.body = newBody;

	Matter.World.add(engine.world, newBody);
}
