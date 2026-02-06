export interface HistoryItem {
    id: string;
    timestamp: number;
    duration: number;
    audioPath: string; // Relative to app data dir or absolute
    transcriptPath: string; // Relative to app data dir or absolute
    title: string;
    previewText: string;
    type?: 'recording' | 'batch';
    searchContent?: string;
}
