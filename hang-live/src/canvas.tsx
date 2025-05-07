import { onCleanup } from "solid-js";
import { Room } from "./room";

const RELAY = new URL("http://localhost:4443");

const Canvas = (props: { path: string }) => {
	const canvas = (<canvas width={window.innerWidth} height={window.innerHeight} />) as HTMLCanvasElement;

	const room = new Room(RELAY, props.path, canvas);

	// Register any window/document level events.
	const resize = () => {
		canvas.width = window.innerWidth;
		canvas.height = window.innerHeight;
	};

	const visible = () => {
		room.visible = document.visibilityState !== "hidden";
	};

	visible();

	window.addEventListener("resize", resize);
	document.addEventListener("visibilitychange", visible);

	onCleanup(() => {
		window.removeEventListener("resize", resize);
		document.removeEventListener("visibilitychange", visible);
	});

	return canvas;
};

export default Canvas;
