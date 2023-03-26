import { Player } from "./transport/index"

const params = new URLSearchParams(window.location.search)

const url = params.get("url") || "https://localhost:4443/watch"
const canvas = document.querySelector<HTMLCanvasElement>("canvas#video")!;

const player = new Player({
    url: url,
    canvas: canvas,
})