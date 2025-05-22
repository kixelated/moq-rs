import "./index.css"
import "./highlight"

import HangSupport from "@kixelated/hang/support/element"
import HangWatch from "@kixelated/hang/watch/element"

export { HangWatch, HangSupport }

const watch = document.querySelector("hang-watch") as HangWatch

// If query params are provided, use it as the broadcast name.
const urlParams = new URLSearchParams(window.location.search)
const name = urlParams.get("name") ?? "demo/bbb"
watch.setAttribute("url", `http://localhost:4443/${name}.hang`)
