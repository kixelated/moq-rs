export { };

declare global {
    interface Window {
        config: AppSettings;
        player: any;
        estimator: any;
    }
    type SWMAThresholdType = 'minimum_duration' | 'percentage';
    type SWMACalculationType = 'segment' | 'window' | 'first_chunk';

    type StreamingStatus = 'paused' | 'streaming';
}


interface AppSettings {
    defaultPlayerURL: string;
    resolutions: { [id: string]: string };
    throttleData: { [id: number]: string };
    serverURL: string;
    activeBWAsset: { url: string; size: number };
    activeBWTestInterval?: number,
    autoStart: boolean;
    testDuration?: number;
    swma_threshold: number;
    swma_calculation_type: SWMACalculationType;
    swma_threshold_type: SWMAThresholdType;
    swma_window_size: number; // sliding window size in terms of chunk count
    swma_calculation_interval: number; // tput is computed each N chunk.
}