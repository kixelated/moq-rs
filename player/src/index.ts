import Player from "./player"

const params = new URLSearchParams(window.location.search)

const url = params.get("url") || "https://localhost:4443/watch"
const canvas = document.querySelector<HTMLCanvasElement>("canvas#video")!

const player = new Player({
    url: url,
    canvas: canvas,
})

const play = document.querySelector<HTMLElement>("#screen #play")!

let playFunc = (e: Event) => {
    player.play()
    e.preventDefault()

    play.removeEventListener('click', playFunc)
    play.style.display = "none"
}

play.addEventListener('click', playFunc)