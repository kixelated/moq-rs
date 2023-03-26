import { Player } from "./player"

// This is so ghetto but I'm too lazy to improve it right now
const videoRef = document.querySelector<HTMLVideoElement>("video#vid")!;
const liveRef = document.querySelector<HTMLElement>("#live")!;
const throttleRef = document.querySelector<HTMLElement>("#throttle")!;
const statsRef = document.querySelector<HTMLElement>("#stats")!;
const playRef = document.querySelector<HTMLElement>("#play")!;

const params = new URLSearchParams(window.location.search)

const url = params.get("url") || "https://localhost:4443/watch"

const player = new Player({
    url: url,
    videoRef: videoRef,
    statsRef: statsRef,
    throttleRef: throttleRef,
})

liveRef.addEventListener("click", (e) => {
    e.preventDefault()
    player.goLive()
})

throttleRef.addEventListener("click", (e) => {
    e.preventDefault()
    player.throttle()
})

playRef.addEventListener('click', (e) => {
    videoRef.play()
    e.preventDefault()
})

function playFunc(e: Event) {
    playRef.style.display = "none"
    //player.goLive()

    // Only fire once to restore pause/play functionality
    videoRef.removeEventListener('play', playFunc)
}

videoRef.addEventListener('play', playFunc)
videoRef.volume = 0.5

// Try to autoplay but ignore errors on mobile; they need to click
//vidRef.play().catch((e) => console.warn(e))