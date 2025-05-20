import "./index.css";
import "./highlight";

import { PublishElement } from "../publish/element";
import { SupportElement } from "../support/element";
export { PublishElement, SupportElement };

const publish = document.querySelector("hang-publish") as PublishElement;
const watch = document.getElementById("watch") as HTMLAnchorElement;
const watchName = document.getElementById("watch-name") as HTMLSpanElement;

const urlParams = new URLSearchParams(window.location.search);
const name = urlParams.get("name") ?? "demo/me";
publish.setAttribute("url", `http://localhost:4443/${name}.hang`);
watch.href = `index.html?name=${name}`;
watchName.textContent = name;
