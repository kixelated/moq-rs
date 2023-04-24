import { Player } from './player';
import * as Plotly from 'plotly.js-dist';
import { estimator } from './estimator';

// This is so ghetto but I'm too lazy to improve it right now
const vidRef = document.getElementById("vid") as HTMLVideoElement;
const startRef = document.getElementById("start") as HTMLButtonElement;
const liveRef = document.getElementById("live") as HTMLButtonElement;
const throttleRef = document.getElementById("throttle") as HTMLButtonElement;
const throttleDDL = document.getElementById("throttles") as HTMLSelectElement;
const statsRef = document.getElementById("stats") as HTMLDivElement;
const playRef = document.getElementById("play") as HTMLDivElement;
const resolutionsRef = document.getElementById("resolutions") as HTMLSelectElement;
const activeBWTestRef = document.getElementById("active_bw_test")
const continueStreamingRef = document.getElementById("continue_streaming")
const logContentRef = document.querySelector("#log_content") as HTMLTextAreaElement;
const toggleLogRef = document.querySelector("#toggle_log") as HTMLAnchorElement;

const params = new URLSearchParams(window.location.search)
window.estimator = estimator;

if (process.env.SERVER_URL) {
    console.log('Setting server url to %s', process.env.SERVER_URL)
    window.config.serverURL = process.env.SERVER_URL
}

// get default values from the querystring if there's any
if (params.get('swma_calculation_type')) {
    window.config.swma_calculation_type = params.get('swma_calculation_type') as SWMACalculationType;
}

if (params.get('swma_threshold_type')) {
    window.config.swma_threshold_type = params.get('swma_threshold_type') as SWMAThresholdType;
}

if (params.get('swma_threshold')) {
    window.config.swma_threshold = parseInt(params.get('swma_threshold') || '5', 10);
}

if (params.get('swma_window_size')) {
    window.config.swma_window_size = parseInt(params.get('swma_window_size') || '50', 10);
}

if (params.get('swma_calculation_interval')) {
    window.config.swma_calculation_interval = parseInt(params.get('swma_calculation_interval') || '10', 10);
}

const logHandler = (txt: string) => {
    const div = document.createElement('div');
    const pre = document.createElement('pre');
    pre.innerText = txt;
    div.appendChild(pre);
    logContentRef.appendChild(div);
};

// fill resolutions combobox
Object.keys(window.config.resolutions).forEach(key => {
    resolutionsRef.options[resolutionsRef.options.length] = new Option(window.config.resolutions[key], key);
});

Object.keys(window.config.throttleData).forEach(key => {
    throttleDDL.options[throttleDDL.options.length] = new Option(window.config.throttleData[parseInt(key)], key);
});

const plotConfig = {
    toImageButtonOptions: {
        format: 'svg', // one of png, svg, jpeg, webp
        filename: 'custom_image',
        width: 700,
        scale: 1 // Multiply title/legend/axis/canvas sizes by this factor
    },
    displayModeBar: true,
    scrollZoom: true,
    displaylogo: false,
    responsive: true
} as Plotly.Config;

const plotLayout = {
    hovermode: 'closest',
    margin: {
        r: 10,
        t: 40,
        b: 40,
        l: 50
    },
    height: 400,
    title: '',
    showlegend: true,
    legend: {
        x: 0,
        y: -0.3,
        orientation: 'h',
    },
    grid: {
        rows: 1,
        columns: 1,
        pattern: 'independent'
    },
    xaxis: {
        anchor: 'y',
        type: 'linear',
        showgrid: true,
        showticklabels: true,
        title: 'Time (s)',
        rangemode: 'tozero'
    },
    yaxis: {
        anchor: 'x',
        showgrid: true,
        title: 'Mbps',
        rangemode: 'tozero'
    },
    font: {
        family: 'sans-serif',
        size: 18,
        color: '#000'
    },

} as Plotly.Layout;

const plotData = [{
    x: [] as number[],
    y: [] as number[],
    name: 'Server ETP',
    mode: 'markers',
    xaxis: 'x',
    yaxis: 'y',
    marker: {
        color: 'black',
        size: 11,
        symbol: 'cross-thin',
        line: {
            width: 3,
        }
    }
}, {
    x: [] as number[],
    y: [] as number[],
    name: 'tc Rate',
    mode: 'line',
    xaxis: 'x',
    yaxis: 'y',
    line: {
        color: '#0905ed',
        width: 3
    }
}, {
    x: [] as number[],
    y: [] as number[],
    name: 'SWMA',
    mode: 'markers',
    xaxis: 'x',
    yaxis: 'y',
    marker: {
        color: '#b33dc6',
        size: 11,
        symbol: 'x-thin',
        line: {
            width: 3,
            color: 'red'
        }
    }
},
{
    x: [] as number[],
    y: [] as number[],
    name: 'IFA',
    mode: 'markers',
    xaxis: 'x',
    yaxis: 'y',
    marker: {
        color: '#037325',
        size: 11,
        symbol: 'star-triangle-down'
    }
},
{
    x: [],
    y: [],
    name: 'Active Bandwidth Test',
    mode: 'markers',
    xaxis: 'x',
    yaxis: 'y',
    marker: {
        size: 7,
        color: '#27aeef'
    },
}
] as any[];

const plot = Plotly.newPlot(document.getElementById('plot') as HTMLDivElement, plotData, plotLayout, plotConfig);

const player = new Player({
    url: params.get("url") || window.config.serverURL,
    vid: vidRef,
    stats: statsRef,
    throttle: throttleRef,
    throttleDDLRef: throttleDDL,
    resolutions: resolutionsRef,
    activeBWTestRef: activeBWTestRef,
    continueStreamingRef: continueStreamingRef,
    activeBWAsset: window.config.activeBWAsset,
    activeBWTestInterval: window.config.activeBWTestInterval,
    autioStart: window.config.autoStart || true,
    logger: logHandler
})

// expose player
window.player = player;


let timePassed = 0;
let playerRefreshInterval = 1000; // 1 second
const displayedHistory = 240; // 4 minutes
const plotStartDelay = 4000; // 4 seconds
const testDuration = window.config.testDuration || 0;

let plotTimer: NodeJS.Timer;

const startPlotting = () => {
    console.log('in startPlotting');
    plotTimer = setInterval(() => {
        if (!player.started || player.paused) {
            return;
        }
        timePassed += playerRefreshInterval;

        const currentSec = Math.round(timePassed / 1000);

        if (testDuration > 0 && currentSec === testDuration) {
            player.pauseOrResume(true);
            player.downloadStats().then(results => {
                console.log('results', results);
            });
            return;
        }

        // save results by time
        // these will be downloaded after the test
        player.saveResultBySecond('swma', player.throughputs.get('swma') || 0, currentSec);
        player.saveResultBySecond('ifa', player.throughputs.get('ifa') || 0, currentSec);
        player.saveResultBySecond('etp', player.serverBandwidth || 0, currentSec);
        player.saveResultBySecond('tcRate', player.tcRate || 0, currentSec);
        player.saveResultBySecond('last-active-bw', player.lastActiveBWTestResult || 0, currentSec);

        plotData.forEach(p => (p.x as Plotly.Datum[]).push(currentSec));
        (plotData[0].y as Plotly.Datum[]).push(player.serverBandwidth / 1000000);
        (plotData[1].y as Plotly.Datum[]).push(player.tcRate / 1000000);
        (plotData[2].y as Plotly.Datum[]).push(player.supress_throughput_value ? null : (player.throughputs.get('swma') || 0) / 1000000);
        (plotData[3].y as Plotly.Datum[]).push(player.supress_throughput_value ? null : (player.throughputs.get('ifa') || 0) / 1000000);
        (plotData[4].y as Plotly.Datum[]).push(player.activeBWTestResult === 0 ? null : player.activeBWTestResult / 1000000);

        // show max 60 seconds
        if (plotData[0].x.length > displayedHistory) {
            plotData.forEach(item => {
                (item.x as Plotly.Datum[]).splice(0, 1);
                (item.y as Plotly.Datum[]).splice(0, 1);
            })
        }

        const data_update = {
            x: Object.values(plotData).map(item => item.x),
            y: Object.values(plotData).map(item => item.y),
        } as Plotly.Data;

        Plotly.update(document.getElementById('plot') as Plotly.Root, data_update, plotLayout)
    }, playerRefreshInterval);
};

startRef.addEventListener("click", async (e) => {
    e.preventDefault();
    if (!player.started) {
        await player.start();
        if (player.started) {
            startRef.innerText = 'Stop';
            setTimeout(() => startPlotting(), plotStartDelay);
        } else {
            alert('Error occurred in starting!');
        }
    } else {
        player.stop();
    }
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
    vidRef.muted = true;
    vidRef.play()
    e.preventDefault()
})

toggleLogRef.addEventListener('click', (e) => {
    const logEl = document.getElementById('log');
    if (!logEl) {
        return;
    };

    if (toggleLogRef.innerText === 'Show Logs') {
        toggleLogRef.innerText = 'Hide Logs';
        logEl.style.display = 'block';
    } else {
        toggleLogRef.innerText = 'Show Logs';
        logEl.style.display = 'none';
    }
});

function playFunc(e: Event) {
    playRef.style.display = "none"
    //player.goLive()

    // Only fire once to restore pause/play functionality
    vidRef.removeEventListener('play', playFunc)
}

vidRef.addEventListener('play', playFunc)
vidRef.volume = 0.5

// Try to autoplay but ignore errors on mobile; they need to click
// vidRef.play().catch((e) => console.warn(e))