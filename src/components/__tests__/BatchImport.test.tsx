import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import { BatchImport } from '../BatchImport';
import { useTranscriptStore } from '../../stores/transcriptStore';

// Mock dependencies
vi.mock('@tauri-apps/api/core', () => ({
    convertFileSrc: vi.fn(),
}));

vi.mock('@tauri-apps/api/event', () => ({
    listen: vi.fn().mockResolvedValue(() => {}),
}));

vi.mock('@tauri-apps/plugin-dialog', () => ({
    open: vi.fn(),
}));

vi.mock('../../services/transcriptionService', () => ({
    transcriptionService: {
        setModelPath: vi.fn(),
        setEnableITN: vi.fn(),
        setITNModelPaths: vi.fn(),
        setPunctuationModelPath: vi.fn(),
        transcribeFile: vi.fn(),
    }
}));

vi.mock('../../services/modelService', () => ({
    modelService: {
        isITNModelInstalled: vi.fn(),
        getITNModelPath: vi.fn(),
    }
}));

vi.mock('react-i18next', () => ({
    useTranslation: () => ({
        t: (key: string) => key,
    }),
}));

describe('BatchImport', () => {
    beforeEach(() => {
        // Reset store state
        useTranscriptStore.setState({
            processingStatus: 'idle',
            processingProgress: 0,
            config: {
                streamingModelPath: '',
                offlineModelPath: '',
                punctuationModelPath: '',
                enableITN: false,
                enabledITNModels: [],
                itnRulesOrder: [],
                theme: 'auto',
                font: 'system',
                language: 'en',
                appLanguage: 'auto'
            }
        });
    });

    afterEach(() => {
        vi.clearAllMocks();
    });

    it('should have accessible progress bar when processing', () => {
        // Set processing state
        useTranscriptStore.setState({
            processingStatus: 'processing',
            processingProgress: 50
        });

        render(<BatchImport />);

        // This is expected to fail initially as the role is missing
        const progressbar = screen.getByRole('progressbar');
        expect(progressbar).toBeDefined();
        expect(progressbar.getAttribute('aria-valuenow')).toBe('50');
        expect(progressbar.getAttribute('aria-valuemin')).toBe('0');
        expect(progressbar.getAttribute('aria-valuemax')).toBe('100');
    });
});
