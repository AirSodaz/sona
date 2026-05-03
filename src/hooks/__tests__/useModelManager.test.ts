import { renderHook, act, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useModelManager } from '../useModelManager';
import { modelService } from '../../services/modelService';
import { useConfigStore } from '../../stores/configStore';
import { useDialogStore } from '../../stores/dialogStore';
import { setTestConfig } from '../../test-utils/configTestUtils';

const SENSEVOICE_INT8_ID = 'sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17';
const SENSEVOICE_FP32_ID = 'sherpa-onnx-sense-voice-zh-en-ja-ko-yue-2024-07-17';
const SILERO_VAD_ID = 'silero-vad';
const catalogModel = {
    id: 'preset-a',
    name: 'Preset A',
    description: 'settings.descriptions.preset_a',
    url: 'https://example.com/preset-a.tar.bz2',
    type: 'sensevoice',
    modes: ['streaming', 'offline'],
    language: 'zh,en',
    size: '1 MB',
    engine: 'sherpa-onnx',
    rules: { requiresVad: false, requiresPunctuation: false },
    installPath: '/models/preset-a',
    downloadPath: '/models/preset-a.tar.bz2',
    isInstalled: true,
} as any;
const modelCatalogSnapshot = {
    modelsDir: '/models',
    models: [catalogModel],
    sections: [
        {
            type: 'asr',
            groups: [
                {
                    key: 'preset-a',
                    models: [catalogModel],
                },
            ],
        },
    ],
} as any;

function buildInstalledCatalogSnapshot(installedIds: string[]): any {
    const models = [
        SENSEVOICE_INT8_ID,
        SENSEVOICE_FP32_ID,
        SILERO_VAD_ID,
    ].map((id) => ({
        ...catalogModel,
        id,
        installPath: `/models/${id}`,
        downloadPath: `/models/${id}.tar.bz2`,
        isInstalled: installedIds.includes(id),
    }));

    return {
        modelsDir: '/models',
        models,
        sections: [
            {
                type: 'asr',
                groups: models.map((model) => ({ key: model.id, models: [model] })),
            },
        ],
    };
}

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
    PRESET_MODELS: [
        { id: 'preset-a', name: 'Preset A', type: 'sensevoice' },
    ],
    modelService: {
        isModelInstalled: vi.fn(),
        getModelPath: vi.fn(),
        downloadModel: vi.fn(),
        checkHardware: vi.fn(),
        deleteModel: vi.fn(),
        getModelRules: vi.fn(),
        getModelCatalogSnapshot: vi.fn(),
    }
}));

describe('useModelManager restoreDefaultModelSettings', () => {
    beforeEach(() => {
        vi.mocked(modelService.isModelInstalled).mockReset();
        vi.mocked(modelService.getModelPath).mockReset();
        vi.mocked(modelService.downloadModel).mockReset();
        vi.mocked(modelService.checkHardware).mockReset();
        vi.mocked(modelService.deleteModel).mockReset();
        vi.mocked(modelService.getModelRules).mockReset();
        vi.mocked(modelService.getModelCatalogSnapshot).mockReset();

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
        vi.mocked(modelService.getModelCatalogSnapshot).mockResolvedValue(modelCatalogSnapshot);
    });

    it('does not load the model catalog while inactive for settings tab prewarm', async () => {
        vi.mocked(modelService.isModelInstalled).mockResolvedValue(false);

        renderHook(() => useModelManager(false));

        await act(async () => {
            await Promise.resolve();
        });

        expect(modelService.getModelCatalogSnapshot).not.toHaveBeenCalled();
        expect(modelService.isModelInstalled).not.toHaveBeenCalled();
    });

    it('loads the model catalog snapshot when the model pane becomes active', async () => {
        vi.mocked(modelService.isModelInstalled).mockResolvedValue(false);

        const { result, rerender } = renderHook(({ isOpen }) => useModelManager(isOpen), {
            initialProps: { isOpen: false },
        });

        rerender({ isOpen: true });

        await waitFor(() => {
            expect(modelService.getModelCatalogSnapshot).toHaveBeenCalledTimes(1);
            expect(result.current.installedModels.has('preset-a')).toBe(true);
        });
        expect(modelService.isModelInstalled).not.toHaveBeenCalled();
    });

    it('defers model catalog loading until after the active frame', async () => {
        vi.mocked(modelService.isModelInstalled).mockResolvedValue(false);

        const { rerender } = renderHook(({ isOpen }) => useModelManager(isOpen), {
            initialProps: { isOpen: false },
        });

        rerender({ isOpen: true });

        expect(modelService.getModelCatalogSnapshot).not.toHaveBeenCalled();
        expect(modelService.isModelInstalled).not.toHaveBeenCalled();

        await waitFor(() => {
            expect(modelService.getModelCatalogSnapshot).toHaveBeenCalledTimes(1);
        });
        expect(modelService.isModelInstalled).not.toHaveBeenCalled();
    });

    it('restores SenseVoice Int8 and Silero VAD when both are installed', async () => {
        vi.mocked(modelService.getModelCatalogSnapshot).mockResolvedValue(
            buildInstalledCatalogSnapshot([SENSEVOICE_INT8_ID, SILERO_VAD_ID])
        );

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
        vi.mocked(modelService.getModelCatalogSnapshot).mockResolvedValue(
            buildInstalledCatalogSnapshot([SENSEVOICE_FP32_ID])
        );

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
        vi.mocked(modelService.getModelCatalogSnapshot).mockResolvedValue(
            buildInstalledCatalogSnapshot([])
        );

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
        vi.mocked(modelService.getModelCatalogSnapshot).mockResolvedValue(
            buildInstalledCatalogSnapshot([SENSEVOICE_INT8_ID])
        );

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
