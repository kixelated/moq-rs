window.config = {
    serverURL: "https://moq.streaming.university:4443",
    resolutions: { 2: "Low (360p)", 1: "Medium (540p)", 0: "High (1080p)" },
    throttleData: {
        209715200: "200Mb/s",
        67108864: "64Mb/s",
        16777216: "16Mb/s",
        4194304: "4Mb/s",
        2097152: "2Mb/s",
        1048576: "1Mb/s",
        524288: "512Kb/s",
        262144: "256Kb/s",
        131072: "128Kb/s",
    },
    activeBWAsset: {
        url: "https://moq.streaming.university/side-load/chunk-stream0-00007.m4s"
    },
    swma_calculation_type: 'segment',
    swma_threshold: 5,
    swma_threshold_type: 'percentage',
    swma_window_size: 50,
    swma_calculation_interval: 10
};
