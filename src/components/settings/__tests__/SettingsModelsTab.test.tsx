import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { SettingsModelsTab } from '../SettingsModelsTab';
import { useConfigStore } from '../../../stores/configStore';
import { ModelManagerContext } from '../../../hooks/useModelManager';
import { setTestConfig } from '../../../test-utils/configTestUtils';

vi.mock('react-i18next', () => ({
    useTranslation: () => ({
        t: (key: string, options?: { defaultValue?: string } & Record<string, unknown>) => {
            if (typeof options?.defaultValue === 'string') {
                return options.defaultValue.replace(/\{\{(\w+)\}\}/g, (_: string, variable: string) => String(options?.[variable] ?? ''));
            }
            return key;
        },
    }),
    initReactI18next: {
        type: '3rdParty',
        init: () => undefined,
    },
}));

vi.mock('../ModelCard', () => ({
    ModelCard: ({ models }: { models: Array<{ id: string; name: string }> }) => (
        <div data-testid={`model-card-${models[0].id}`}>{models[0].name}</div>
    ),
}));

vi.mock('../../../services/modelService', () => ({
    PRESET_MODELS: [],
    PRESET_MODELS_MAP: {},
    modelService: {
        getModelPath: vi.fn(async (id: string) => `/models/${id}`),
        getModelRules: vi.fn(() => ({ requiresVad: false, requiresPunctuation: false })),
    },
}));

const speakerSegmentationModelBase = {
    id: 'sherpa-onnx-pyannote-segmentation-3-0',
    name: 'Pyannote 3.0',
    description: 'settings.descriptions.speaker_segmentation',
    url: 'https://example.com/seg.tar.bz2',
    type: 'speaker-segmentation',
    language: 'multi',
    size: '6.64 MB',
    engine: 'sherpa-onnx',
    rules: { requiresVad: false, requiresPunctuation: false },
    installPath: '/models/sherpa-onnx-pyannote-segmentation-3-0',
    downloadPath: '/models/sherpa-onnx-pyannote-segmentation-3-0.tar.bz2',
};
const speakerEmbeddingModelBase = {
    id: '3dspeaker_speech_campplus_sv_zh_en_16k-common_advanced.onnx',
    name: '3DSpeaker CAMPPlus',
    description: 'settings.descriptions.speaker_embedding',
    url: 'https://example.com/embed.onnx',
    type: 'speaker-embedding',
    language: 'zh,en',
    size: '27 MB',
    isArchive: false,
    filename: '3dspeaker_speech_campplus_sv_zh_en_16k-common_advanced.onnx',
    engine: 'sherpa-onnx',
    rules: { requiresVad: false, requiresPunctuation: false },
    installPath: '/models/3dspeaker_speech_campplus_sv_zh_en_16k-common_advanced.onnx',
    downloadPath: '/models/3dspeaker_speech_campplus_sv_zh_en_16k-common_advanced.onnx',
};

function buildModelCatalog(installedModels: Set<string>) {
    const speakerSegmentationModel = {
        ...speakerSegmentationModelBase,
        isInstalled: installedModels.has(speakerSegmentationModelBase.id),
    };
    const speakerEmbeddingModel = {
        ...speakerEmbeddingModelBase,
        isInstalled: installedModels.has(speakerEmbeddingModelBase.id),
    };

    return {
        modelsDir: '/models',
        models: [speakerSegmentationModel, speakerEmbeddingModel],
        sections: [
        {
            type: 'speaker-segmentation',
            groups: [
                {
                    key: 'sherpa-onnx-pyannote-segmentation-3-0',
                    models: [speakerSegmentationModel],
                },
            ],
        },
        {
            type: 'speaker-embedding',
            groups: [
                {
                    key: '3dspeaker_speech_campplus_sv_zh_en_16k-common_advanced.onnx',
                    models: [speakerEmbeddingModel],
                },
            ],
        },
        ],
        selectionOptions: {
            streaming: [],
            offline: [],
            speakerSegmentation: [
                {
                    id: speakerSegmentationModel.id,
                    label: speakerSegmentationModel.name,
                    installPath: speakerSegmentationModel.installPath,
                    isInstalled: speakerSegmentationModel.isInstalled,
                },
            ],
            speakerEmbedding: [
                {
                    id: speakerEmbeddingModel.id,
                    label: speakerEmbeddingModel.name,
                    installPath: speakerEmbeddingModel.installPath,
                    isInstalled: speakerEmbeddingModel.isInstalled,
                },
            ],
        },
        modelPathById: {
            [speakerSegmentationModel.id]: speakerSegmentationModel.installPath,
            [speakerEmbeddingModel.id]: speakerEmbeddingModel.installPath,
        },
        modelIdByNormalizedPath: {
            [speakerSegmentationModel.installPath.toLowerCase()]: speakerSegmentationModel.id,
            [speakerEmbeddingModel.installPath.toLowerCase()]: speakerEmbeddingModel.id,
        },
        pathMatchTokens: [
            {
                id: speakerSegmentationModel.id,
                token: speakerSegmentationModel.id.toLowerCase(),
            },
            {
                id: speakerEmbeddingModel.id,
                token: speakerEmbeddingModel.id.toLowerCase(),
            },
        ],
        dependencyRequestsByModelId: {},
        restoreDefaults: {
            punctuationModelPath: '',
            speakerSegmentationModelPath: '',
            speakerEmbeddingModelPath: '',
            enableITN: true,
            vadBufferSize: 5,
            maxConcurrent: 2,
        },
    };
}

function renderTab(installedModels: Set<string>) {
    function Harness() {
        const config = useConfigStore((state) => state.config);
        const managerValue = {
            deletingId: null,
            downloads: {},
            installedModels,
            modelCatalog: buildModelCatalog(installedModels),
            selectedModelIds: {
                streaming: null,
                offline: null,
                speakerSegmentation: config.speakerSegmentationModelPath ? speakerSegmentationModelBase.id : null,
                speakerEmbedding: config.speakerEmbeddingModelPath ? speakerEmbeddingModelBase.id : null,
            },
            handleDelete: vi.fn(),
            handleDownload: vi.fn(),
            handleCancelDownload: vi.fn(),
            handleLoad: vi.fn(),
            restoreDefaultModelSettings: vi.fn(),
        } as any;

        return (
            <ModelManagerContext.Provider value={managerValue}>
                <SettingsModelsTab />
            </ModelManagerContext.Provider>
        );
    }

    return render(<Harness />);
}

describe('SettingsModelsTab speaker model selections', () => {
    beforeEach(() => {
        vi.clearAllMocks();
        setTestConfig({
            speakerSegmentationModelPath: '',
            speakerEmbeddingModelPath: '',
        });
    });

    it('shows Off for both speaker dropdowns even when no speaker models are installed', async () => {
        renderTab(new Set());

        await waitFor(() => {
            expect(screen.getByRole('button', { name: 'Speaker Segmentation Model' }).textContent).toContain('Off');
            expect(screen.getByRole('button', { name: 'Speaker Embedding Model' }).textContent).toContain('Off');
        });

        // Expand accordions to mount ModelCards
        const segAccordion = screen.getByRole('button', { name: /Speaker Segmentation Models/ });
        fireEvent.click(segAccordion);

        const embedAccordion = screen.getByRole('button', { name: /Speaker Embedding Models/ });
        fireEvent.click(embedAccordion);

        expect(screen.getByTestId('model-card-sherpa-onnx-pyannote-segmentation-3-0').textContent).toContain('Pyannote 3.0');
        expect(screen.getByTestId('model-card-3dspeaker_speech_campplus_sv_zh_en_16k-common_advanced.onnx').textContent).toContain('3DSpeaker CAMPPlus');

        fireEvent.click(screen.getByRole('button', { name: 'Speaker Segmentation Model' }));
        expect(screen.getAllByRole('option').map((option) => option.textContent)).toEqual(['Off']);
    });

    it('clears selected speaker model paths when Off is chosen', async () => {
        setTestConfig({
            speakerSegmentationModelPath: '/models/sherpa-onnx-pyannote-segmentation-3-0',
            speakerEmbeddingModelPath: '/models/3dspeaker_speech_campplus_sv_zh_en_16k-common_advanced.onnx',
        });

        renderTab(new Set([
            'sherpa-onnx-pyannote-segmentation-3-0',
            '3dspeaker_speech_campplus_sv_zh_en_16k-common_advanced.onnx',
        ]));

        await waitFor(() => {
            expect(screen.getByRole('button', { name: 'Speaker Segmentation Model' }).textContent).toContain('Pyannote 3.0');
            expect(screen.getByRole('button', { name: 'Speaker Embedding Model' }).textContent).toContain('3DSpeaker CAMPPlus');
        });

        fireEvent.click(screen.getByRole('button', { name: 'Speaker Segmentation Model' }));
        fireEvent.click(screen.getByRole('option', { name: 'Off' }));

        fireEvent.click(screen.getByRole('button', { name: 'Speaker Embedding Model' }));
        fireEvent.click(screen.getByRole('option', { name: 'Off' }));

        await waitFor(() => {
            expect(useConfigStore.getState().config.speakerSegmentationModelPath).toBe('');
            expect(useConfigStore.getState().config.speakerEmbeddingModelPath).toBe('');
            expect(screen.getByRole('button', { name: 'Speaker Segmentation Model' }).textContent).toContain('Off');
            expect(screen.getByRole('button', { name: 'Speaker Embedding Model' }).textContent).toContain('Off');
        });
    });

    it('allows selecting Volcengine Doubao cloud ASR even when no local ASR model is installed', async () => {
        renderTab(new Set());

        fireEvent.click(screen.getByRole('button', { name: 'settings.select_streaming_model' }));
        fireEvent.click(screen.getByRole('option', { name: '豆包语音 (云端)' }));

        await waitFor(() => {
            const config = useConfigStore.getState().config;
            expect(config.streamingModelPath).toBe('');
            expect(config.asr?.selections.live).toMatchObject({
                engine: 'online',
                mode: 'streaming',
                modelPath: '',
                providerId: 'volcengine-doubao',
                profileId: 'volcengine-doubao-default',
            });
            expect(config.asr?.selections.caption.engine).toBe('online');
            expect(config.asr?.selections.voiceTyping.engine).toBe('online');
        });

        expect(screen.queryByText('音频会发送到火山引擎进行识别。')).not.toBeNull();
    });

    it('keeps a selected Volcengine batch slot when local ASR models are not installed', async () => {
        setTestConfig({
            asr: {
                selections: {
                    live: { engine: 'local-sherpa', mode: 'streaming', modelId: null, modelPath: '' },
                    caption: { engine: 'local-sherpa', mode: 'streaming', modelId: null, modelPath: '' },
                    voiceTyping: { engine: 'local-sherpa', mode: 'streaming', modelId: null, modelPath: '' },
                    batch: {
                        engine: 'online',
                        mode: 'offline',
                        modelId: null,
                        modelPath: '',
                        providerId: 'volcengine-doubao',
                        profileId: 'volcengine-doubao-default',
                    },
                },
            },
            offlineModelPath: '',
        } as any);

        renderTab(new Set());

        await waitFor(() => {
            expect(screen.getByRole('button', { name: '豆包语音 (云端)' })).not.toBeNull();
            expect(useConfigStore.getState().config.asr?.selections.batch.engine).toBe('online');
        });
    });

    it('keeps Volcengine local batch import on flash mode and disables async URL-only modes', async () => {
        setTestConfig({
            asr: {
                selections: {
                    live: { engine: 'local-sherpa', mode: 'streaming', modelId: null, modelPath: '' },
                    caption: { engine: 'local-sherpa', mode: 'streaming', modelId: null, modelPath: '' },
                    voiceTyping: { engine: 'local-sherpa', mode: 'streaming', modelId: null, modelPath: '' },
                    batch: {
                        engine: 'online',
                        mode: 'offline',
                        modelId: null,
                        modelPath: '',
                        providerId: 'volcengine-doubao',
                        profileId: 'volcengine-doubao-default',
                    },
                },
                providers: {
                    online: {
                        'volcengine-doubao': {
                            apiKey: 'volc-test-key',
                            streamingEndpoint: 'wss://openspeech.bytedance.com/api/v3/sauc/bigmodel_async',
                            streamingResourceId: 'volc.seedasr.sauc.duration',
                            batchEndpoint: 'https://openspeech.bytedance.com/api/v3/auc/bigmodel/recognize/flash',
                            batchResourceId: 'volc.bigasr.auc_turbo',
                        },
                    },
                },
            },
        } as any);

        renderTab(new Set());

        fireEvent.click(screen.getByRole('button', { name: '急速 (同步直回)' }));

        const standardOption = screen.getByRole('option', { name: /普通 \(异步轮询\)/ });
        const offpeakOption = screen.getByRole('option', { name: /闲时 \(特惠异步\)/ });

        expect((standardOption as HTMLButtonElement).disabled).toBe(true);
        expect((offpeakOption as HTMLButtonElement).disabled).toBe(true);
        expect(screen.getAllByText('需要公网音频 URL，当前本地批量导入暂不支持。').length).toBeGreaterThan(0);

        fireEvent.click(standardOption);

        await waitFor(() => {
            expect(useConfigStore.getState().config.asr?.providers?.online?.['volcengine-doubao']).toMatchObject({
                batchEndpoint: 'https://openspeech.bytedance.com/api/v3/auc/bigmodel/recognize/flash',
                batchResourceId: 'volc.bigasr.auc_turbo',
            });
        });
    });
});
