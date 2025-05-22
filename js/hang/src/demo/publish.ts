import "./index.css"
import "./highlight"

import HangPublish from "../publish/element"
import HangSupport from "../support/element"
export { HangPublish, HangSupport }

const publish = document.querySelector("hang-publish") as HangPublish
const watch = document.getElementById("watch") as HTMLAnchorElement
const watchName = document.getElementById("watch-name") as HTMLSpanElement

const urlParams = new URLSearchParams(window.location.search)
const name = urlParams.get("name") ?? "demo/me"
publish.setAttribute("url", `http://localhost:4443/${name}.hang`)
watch.href = `index.html?name=${name}`
watchName.textContent = name
