import Matter from "matter-js";

// Canvas setup
const canvas = document.getElementById("canvas") as HTMLCanvasElement;
function resizeCanvas() {
	canvas.width = window.innerWidth;
	canvas.height = window.innerHeight;
}

window.addEventListener("resize", resizeCanvas);
resizeCanvas(); // initial call

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
	originalW: number;
	originalH: number;
	body: Matter.Body;
}

// SETUP PHYSICS ENGINE (no gravity)
const engine = Matter.Engine.create();
engine.gravity.x = 0;
engine.gravity.y = 0;

const boxes: Box[] = [
	{ id: "A", w: 100, h: 100, targetX: 0.2, targetY: 0.2, originalW: 100, originalH: 100 },
	{ id: "B", w: 120, h: 100, targetX: 0.4, targetY: 0.25, originalW: 120, originalH: 100 },
	{ id: "C", w: 80, h: 80, targetX: 0.3, targetY: 0.3, originalW: 80, originalH: 80 },
	{ id: "D", w: 90, h: 90, targetX: 0.35, targetY: 0.2, originalW: 90, originalH: 90 },
].map((p) => ({
	...p,
	body: Matter.Bodies.rectangle(p.targetX * canvas.width, p.targetY * canvas.height, p.w, p.h, {
		inertia: Number.POSITIVE_INFINITY,
		restitution: 0.5,
		frictionAir: 0.2,
	}),
}));

Matter.World.add(
	engine.world,
	boxes.map((b) => b.body),
);

// Simulation loop
function tick() {
	for (const box of boxes) {
		const body = box.body;
		const dx = box.targetX * canvas.width - body.position.x;
		const dy = box.targetY * canvas.height - body.position.y;
		const stiffness = dragging && dragging.box.id === box.id ? 0.002 : 0.0005;
		Matter.Body.applyForce(body, body.position, { x: dx * stiffness, y: dy * stiffness });

		// Boundary repulsion
		const boundary = 20;
		const left = boundary + box.w / 2;
		const right = canvas.width - boundary - box.w / 2;
		const top = boundary + box.h / 2;
		const bottom = canvas.height - boundary - box.h / 2;

		if (body.position.x < left) {
			Matter.Body.applyForce(body, body.position, { x: (left - body.position.x) * 0.004, y: 0 });
		} else if (body.position.x > right) {
			Matter.Body.applyForce(body, body.position, { x: (right - body.position.x) * 0.004, y: 0 });
		}

		if (body.position.y < top) {
			Matter.Body.applyForce(body, body.position, { x: 0, y: (top - body.position.y) * 0.004 });
		} else if (body.position.y > bottom) {
			Matter.Body.applyForce(body, body.position, { x: 0, y: (bottom - body.position.y) * 0.004 });
		}
	}

	Matter.Engine.update(engine, 1000 / 60);
	render();
	requestAnimationFrame(tick);
}

// Render loop
function render() {
	ctx.clearRect(0, 0, canvas.width, canvas.height);

	for (const box of boxes) {
		const { x, y } = box.body.position;
		const angle = box.body.angle;

		ctx.save();
		ctx.translate(x, y);
		ctx.rotate(angle);
		ctx.fillStyle = "#4b9";
		ctx.fillRect(-box.w / 2, -box.h / 2, box.w, box.h);
		ctx.strokeStyle = "#000";
		ctx.strokeRect(-box.w / 2, -box.h / 2, box.w, box.h);
		ctx.fillStyle = "#000";
		ctx.fillText(box.id, -box.w / 2 + 4, -box.h / 2 + 14);
		ctx.restore();

		// Draw target
		ctx.beginPath();
		ctx.arc(box.targetX * canvas.width, box.targetY * canvas.height, 4, 0, 2 * Math.PI);
		ctx.fillStyle = "#f00";
		ctx.fill();
	}

	// Draw bounding box
	ctx.strokeStyle = "#ccc";
	ctx.lineWidth = 2;
	ctx.strokeRect(20, 20, canvas.width - 40, canvas.height - 40);
}

let prevScale = 1;

function updateScale() {
	const padding = 20;
	const usableW = canvas.width - 2 * padding;
	const usableH = canvas.height - 2 * padding;
	const canvasArea = usableW * usableH;

	const totalBoxArea = boxes.reduce((sum, b) => sum + b.w * b.h, 0);

	const fillRatio = totalBoxArea / canvasArea;
	const targetFill = 0.5;

	const scale = Math.sqrt(targetFill / fillRatio);

	prevScale = scale;

	// Apply scale to each box
	for (const box of boxes) {
		box.w = Math.max(20, box.w * scale);
		box.h = Math.max(20, box.h * scale);

		// Resize body
		const { x, y } = box.body.position;
		Matter.World.remove(engine.world, box.body);
		const newBody = Matter.Bodies.rectangle(x, y, box.w, box.h, {
			inertia: Number.POSITIVE_INFINITY,
			restitution: 0.5,
			frictionAir: 0.2,
		});
		Matter.Body.setVelocity(newBody, box.body.velocity);
		Matter.Body.setAngle(newBody, box.body.angle);
		Matter.Body.setAngularVelocity(newBody, box.body.angularVelocity);

		box.body = newBody;
		Matter.World.add(engine.world, box.body);
	}
}

function resizeBody(box: Box, newW: number, newH: number) {
	const { position, velocity, angle, angularVelocity } = box.body;

	Matter.World.remove(engine.world, box.body);

	const newBody = Matter.Bodies.rectangle(position.x, position.y, newW, newH, {
		inertia: Number.POSITIVE_INFINITY,
		frictionAir: 0.2,
	});

	Matter.Body.setVelocity(newBody, velocity);
	Matter.Body.setAngle(newBody, angle);
	Matter.Body.setAngularVelocity(newBody, angularVelocity);

	box.body = newBody;
	box.w = newW;
	box.h = newH;

	Matter.World.add(engine.world, newBody);
	//updateScale();
}

tick();

let dragging: {
	box: (typeof boxes)[0];
} | null = null;

canvas.addEventListener("mousedown", (e) => {
	const rect = canvas.getBoundingClientRect();
	const mx = e.clientX - rect.left;
	const my = e.clientY - rect.top;

	for (const box of boxes) {
		const { x, y } = box.body.position;
		const left = x - box.w / 2;
		const right = x + box.w / 2;
		const top = y - box.h / 2;
		const bottom = y + box.h / 2;

		if (mx >= left && mx <= right && my >= top && my <= bottom) {
			dragging = {
				box,
			};
			break;
		}
	}
});

canvas.addEventListener("mousemove", (e) => {
	if (dragging) {
		const rect = canvas.getBoundingClientRect();
		const mx = e.clientX - rect.left;
		const my = e.clientY - rect.top;
		dragging.box.targetX = mx / canvas.width;
		dragging.box.targetY = my / canvas.height;
	}
});

canvas.addEventListener("mouseup", () => {
	dragging = null;
});

canvas.addEventListener("mouseleave", () => {
	dragging = null;
});

canvas.addEventListener("wheel", (e) => {
	let selectedBox: Box | null = null;
	if (dragging) {
		selectedBox = dragging.box;
	} else {
		const rect = canvas.getBoundingClientRect();
		const mx = e.clientX - rect.left;
		const my = e.clientY - rect.top;

		for (const box of boxes) {
			const { x, y } = box.body.position;
			const left = x - box.w / 2;
			const right = x + box.w / 2;
			const top = y - box.h / 2;
			const bottom = y + box.h / 2;

			if (mx >= left && mx <= right && my >= top && my <= bottom) {
				selectedBox = box;
				break;
			}
		}

		if (!selectedBox) return;
	}

	e.preventDefault(); // Prevent scroll

	const zoomSpeed = 0.01;
	const scale = 1 - e.deltaY * zoomSpeed;

	const newW = selectedBox.w * scale;
	const newH = selectedBox.h * scale;

	if (newW < 20 || newH > canvas.width - 20) {
		return;
	}

	if (newH < 20 || newW > canvas.height - 20) {
		return;
	}

	resizeBody(selectedBox, newW, newH);
});
