import { renderHook, act } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useModelManager } from '../useModelManager';
import { modelService } from '../../services/modelService';
import { useConfigStore } from '../../stores/configStore';
import { useDialogStore } from '../../stores/dialogStore';
import { setTestConfig } from '../../test-utils/configTestUtils';

const SENSEVOICE_INT8_ID = 'sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17';
const SENSEVOICE_FP32_ID = 'sherpa-onnx-sense-voice-zh-en-ja-ko-yue-2024-07-17';
const SILERO_VAD_ID = 'silero-vad';

vi.mock('react-i18next', () => ({
    useTranslation: () => ({
        t: (key: string) => key,
    }),
    initReactI18next: {
        type: '3rdParty',
        init: () => undefined,
    },
}));

vi.mock('../../services/modelService', () => ({
    PRESET_MODELS: [],
    modelService: {
        isModelInstalled: vi.fn(),
        getModelPath: vi.fn(),
        downloadModel: vi.fn(),
        checkHardware: vi.fn(),
        deleteModel: vi.fn(),
        getModelRules: vi.fn(),
    }
}));

describe('useModelManager restoreDefaultModelSettings', () => {
    beforeEach(() => {
        vi.clearAllMocks();

        setTestConfig({
            streamingModelPath: '/current/live',
            offlineModelPath: '/current/offline',
            vadModelPath: '/current/vad',
            punctuationModelPath: '/current/punctuation',
            speakerSegmentationModelPath: '/current/speaker-segmentation',
            speakerEmbeddingModelPath: '/current/speaker-embedding',
            enableITN: false,
            vadBufferSize: 9,
            maxConcurrent: 4,
        });

        useDialogStore.setState({
            isOpen: false,
            options: null,
            resolveRef: null,
            confirm: vi.fn().mockResolvedValue(true),
            showError: vi.fn().mockResolvedValue(undefined),
        });

        vi.mocked(modelService.getModelPath).mockImplementation(async (id: string) => `/models/${id}`);
    });

    it('restores SenseVoice Int8 and Silero VAD when both are installed', async () => {
        vi.mocked(modelService.isModelInstalled).mockImplementation(async (id: string) => (
            id === SENSEVOICE_INT8_ID || id === SILERO_VAD_ID
        ));

        const { result } = renderHook(() => useModelManager(false));

        await act(async () => {
            await result.current.restoreDefaultModelSettings();
        });

        expect(useConfigStore.getState().config).toMatchObject({
            streamingModelPath: `/models/${SENSEVOICE_INT8_ID}`,
            offlineModelPath: `/models/${SENSEVOICE_INT8_ID}`,
            vadModelPath: `/models/${SILERO_VAD_ID}`,
            punctuationModelPath: '',
            speakerSegmentationModelPath: '',
            speakerEmbeddingModelPath: '',
            enableITN: true,
            vadBufferSize: 5,
            maxConcurrent: 2,
        });
    });

    it('falls back to SenseVoice Fp32 when Int8 is unavailable', async () => {
        vi.mocked(modelService.isModelInstalled).mockImplementation(async (id: string) => (
            id === SENSEVOICE_FP32_ID
        ));

        const { result } = renderHook(() => useModelManager(false));

        await act(async () => {
            await result.current.restoreDefaultModelSettings();
        });

        expect(useConfigStore.getState().config).toMatchObject({
            streamingModelPath: `/models/${SENSEVOICE_FP32_ID}`,
            offlineModelPath: `/models/${SENSEVOICE_FP32_ID}`,
            vadModelPath: '/current/vad',
            punctuationModelPath: '',
            speakerSegmentationModelPath: '',
            speakerEmbeddingModelPath: '',
            enableITN: true,
            vadBufferSize: 5,
            maxConcurrent: 2,
        });
    });

    it('keeps the current ASR models when neither SenseVoice default is installed', async () => {
        vi.mocked(modelService.isModelInstalled).mockResolvedValue(false);

        const { result } = renderHook(() => useModelManager(false));

        await act(async () => {
            await result.current.restoreDefaultModelSettings();
        });

        expect(useConfigStore.getState().config).toMatchObject({
            streamingModelPath: '/current/live',
            offlineModelPath: '/current/offline',
            vadModelPath: '/current/vad',
            punctuationModelPath: '',
            speakerSegmentationModelPath: '',
            speakerEmbeddingModelPath: '',
            enableITN: true,
            vadBufferSize: 5,
            maxConcurrent: 2,
        });
    });

    it('keeps the current VAD model when Silero VAD is not installed', async () => {
        vi.mocked(modelService.isModelInstalled).mockImplementation(async (id: string) => (
            id === SENSEVOICE_INT8_ID
        ));

        const { result } = renderHook(() => useModelManager(false));

        await act(async () => {
            await result.current.restoreDefaultModelSettings();
        });

        expect(useConfigStore.getState().config).toMatchObject({
            streamingModelPath: `/models/${SENSEVOICE_INT8_ID}`,
            offlineModelPath: `/models/${SENSEVOICE_INT8_ID}`,
            vadModelPath: '/current/vad',
            punctuationModelPath: '',
            speakerSegmentationModelPath: '',
            speakerEmbeddingModelPath: '',
            enableITN: true,
            vadBufferSize: 5,
            maxConcurrent: 2,
        });
    });
});
