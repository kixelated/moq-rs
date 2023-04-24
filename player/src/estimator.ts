import { dbStore } from './db';

const chunkStats: any[] = [];
let totalChunkCount = 0;
let tputs: any[] = [];

const getSWMAThreshold = () => {
    return window.config.swma_threshold || 5;
}

const getSWMACalculationType = () => {
    return window.config.swma_calculation_type;
}

const getSWMAThresholdType = () => {
    return window.config.swma_threshold_type || 'percentage'
};

const getSWMACalculationInterval = () => {
    return window.config.swma_calculation_interval || 10;
}

const getSWMAWindowSize = () => {
    return window.config.swma_window_size || 50;
}

const initDb = async () => {
    try {
        console.log('initing db');
        if (!await dbStore.init()) {
            console.log('db already inited');
        } else {
            console.log('db inited');
        }
    } catch (ex) {
        alert('db store could not be created');
        console.error(ex);
        return;
    }
}

const getTests = async () => {
    await initDb();
    const tests = await dbStore.getTests();
    return tests;
}
const computeByTestId = async (testId: string) => {
    await initDb();
    const logs = await dbStore.getLogs(testId);
    if (logs?.length > 0) {
        tputs.splice(0);
        let lastTPut = 0;
        logs.forEach((item, i) => {
            totalChunkCount++;
            chunkStats.push([item.chunkSize, item.chunkDownloadDuration]);
            if (totalChunkCount >= getSWMAWindowSize() && totalChunkCount % getSWMACalculationInterval() === 0) {
                let stats = chunkStats.slice(-getSWMAWindowSize());
                let filteredStats = filterStats(stats, getSWMAThreshold(), getSWMAThresholdType(), lastTPut);
                // this.throughputs.set('swma', computeTPut(filteredStats));
                const tput = computeTPut(filteredStats);
                
                filteredStats = filterStats(stats, getSWMAThreshold(), getSWMAThresholdType());
                const tput2 = computeTPut(filteredStats);

                tputs.push([
                    tput,
                    tput2,
                    item.msg_tc_rate * 1000,
                    !item.msg_tc_rate ? 0 : Math.round(Math.abs(tput - item.msg_tc_rate * 1000)), // abs diff
                    !item.msg_tc_rate ? 0 : Math.round(Math.abs(tput - item.msg_tc_rate * 1000) / (item.msg_tc_rate * 1000) * 100), // abs diff percent
                    !item.msg_tc_rate ? 0 : Math.round(Math.abs(tput2 - item.msg_tc_rate * 1000)), // abs diff
                    !item.msg_tc_rate ? 0 : Math.round(Math.abs(tput2 - item.msg_tc_rate * 1000) / (item.msg_tc_rate * 1000) * 100) // abs diff percent
                ]);
                lastTPut = tput;
                console.log('%d %f %f', (i + 1), tput, tput2);
            }
        });
        
        const totalAbsDiff = tputs.reduce((prev: number, current: any) => {
            return (prev || 0) + current[3];
        }, 0);

        const totalAbsDiffPercentage = tputs.reduce((prev: number, current: any) => {
            return (prev || 0) + current[4]; // percentage diff
        }, 0);

        const totalAbsDiff_2 = tputs.reduce((prev: number, current: any) => {
            return (prev || 0) + current[5];
        }, 0);

        const totalAbsDiffPercentage_2 = tputs.reduce((prev: number, current: any) => {
            return (prev || 0) + current[6]; // percentage diff
        }, 0);

        const meanAbsDiff = Math.round(totalAbsDiff / tputs.length);
        const meanAbsDiffPercentage = Math.round(totalAbsDiffPercentage / tputs.length);
        const meanAbsDiff_2 = Math.round(totalAbsDiff_2 / tputs.length);
        const meanAbsDiffPercentage_2 = Math.round(totalAbsDiffPercentage_2 / tputs.length);

        // console.log(tputs.map(x => x.join(',')).join('\n'));
        // console.log('totalAbsDiff: %f totalAbsDiffPercentage: %f meanAbsDiff: %f meanAbsDiffPercentage: %f total: %d', totalAbsDiff, totalAbsDiffPercentage, meanAbsDiff, meanAbsDiffPercentage, tputs.length);
        console.log('meanAbsDiff: %f meanAbsDiffPercentage: %f meanAbsDiff_2: %f meanAbsDiffPercentage_2: %f total: %d', meanAbsDiff, meanAbsDiffPercentage, meanAbsDiff_2, meanAbsDiffPercentage_2, tputs.length);
    }
}

const filterStats = (chunkStats: any[], threshold: number, thresholdType: string, lastTPut?: number) => {
    // filter out the ones with zero download duration
    let filteredStats = chunkStats.slice().filter(a => a[1] > 0);
    console.log('computeTPut | chunk count: %d thresholdType: %s threshold: %d', filteredStats.length, thresholdType, threshold);

    if (threshold > 0 && threshold < 100) {
        // sort chunk by download rate, in descending order
        filteredStats.sort((a, b) => {
            return (a[1] ? a[0] / a[1] : 0) > (b[1] ? b[0] / b[1] : 0) ? -1 : 1;
        });

        const topCut = Math.ceil(threshold / 100 * filteredStats.length);
        const bottomCut = Math.floor(threshold / 100 * filteredStats.length);

        filteredStats.splice(0, topCut);
        filteredStats.splice(filteredStats.length - bottomCut, bottomCut);
    }

    if (lastTPut) {
        const magicalNumber = 5;
        filteredStats = filteredStats.filter(m => {
            const chunkTPut = m[0] * 8 * 1000 / m[1];
            // if lastTPut is less than n time chunk tput or n times lastTPut is greater than chunk tput
            if (lastTPut < magicalNumber * chunkTPut && lastTPut * magicalNumber > chunkTPut) {
                return true;
            } else {
                console.log('filter %f %f', chunkTPut, lastTPut);
                return false;
            };
        });
    }

    console.log('computeTPut | after filtering: chunk count: %d', filteredStats.length);
    return filteredStats;
}

const computeTPut = (stats: any[]) => {
    let totalSize = 0;
    let totalDuration = 0;
    stats.forEach((arr, i) => {
        const size = arr[0];
        const downloadDurationOfChunk = arr[1];
        if (size > 0 && downloadDurationOfChunk > 0) {
            totalSize += size;
            totalDuration += downloadDurationOfChunk;
        }
    });
    return totalSize * 8 * 1000 / totalDuration;
};

export const estimator = {
    computeByTestId,
    getTests
};


