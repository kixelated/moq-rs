export { };

declare global {
    interface Window {
        config: AppSettings;
        player: any;
    }
}

interface AppSettings {
    defaultPlayerURL: string;
    resolutions: { [id: string]: string };
    throttleData: { [id: number]: string };
    serverURL: string;
    activeBWAsset: { url: string; size: number }
    swma_threshold: number;
    swma_threshold_type: 'minimum_duration' | 'percentage';
}