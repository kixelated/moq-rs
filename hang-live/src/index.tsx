/* @refresh reload */
import { render } from "solid-js/web";
import Canvas from "./canvas";
import "./index.css";

const root = document.querySelector("#root");
if (!root) {
	throw new Error("Root element not found");
}

const Main = () => {
	return (
		<>
			<Canvas path="demo/" />

			<div class="controls" style={{ position: "fixed", bottom: 0, left: 0, right: 0 }}>
				<button type="button">🎤</button>
				<button type="button">📷</button>
				<button type="button">🖥️</button>
				<input type="text" placeholder="Type a message..." style="flex-grow: 1;" />
				<button type="button">⚙️</button>
				<button type="button">⛶</button>
			</div>
		</>
	);
};

render(() => <Main />, root);
