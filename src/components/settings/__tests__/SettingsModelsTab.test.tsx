import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
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
    ModelCard: ({
        models,
        actionsDisabled,
    }: {
        models: Array<{ id: string; name: string }>;
        actionsDisabled?: boolean;
    }) => (
        <div data-testid={`model-card-${models[0].id}`}>
            {models[0].name}
            <button type="button" disabled={actionsDisabled}>model-action</button>
        </div>
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

function renderTab(installedModels: Set<string>, managerOverrides: Record<string, unknown> = {}) {
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
            catalogLoadState: 'ready',
            catalogLoadError: null,
            handleDelete: vi.fn(),
            handleDownload: vi.fn(),
            handleCancelDownload: vi.fn(),
            handleLoad: vi.fn(),
            restoreDefaultModelSettings: () => {
                useConfigStore.getState().setConfig({
                    batchVadEnabled: true,
                    punctuationModelPath: '',
                    speakerSegmentationModelPath: '',
                    speakerEmbeddingModelPath: '',
                    enableITN: true,
                    vadBufferSize: 5,
                    maxConcurrent: 2,
                });
            },
            ...managerOverrides,
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
            batchVadEnabled: true,
        });
    });

    it('toggles the batch VAD setting from the transcription settings section', async () => {
        renderTab(new Set());

        expect(screen.getByText('settings.batch_vad_enabled')).toBeDefined();
        expect(screen.getByText('settings.batch_vad_enabled_hint')).toBeDefined();

        const row = screen.getByText('settings.batch_vad_enabled').closest('.settings-item-container');
        expect(row).not.toBeNull();
        const batchVadSwitch = within(row as HTMLElement).getByRole('switch');
        expect(batchVadSwitch.getAttribute('aria-checked')).toBe('true');

        fireEvent.click(batchVadSwitch);

        await waitFor(() => {
            expect(useConfigStore.getState().config.batchVadEnabled).toBe(false);
            expect(batchVadSwitch.getAttribute('aria-checked')).toBe('false');
        });
    });

    it('restores batch VAD setting to true when restoring default settings', async () => {
        setTestConfig({
            batchVadEnabled: false,
        });

        renderTab(new Set());

        const row = screen.getByText('settings.batch_vad_enabled').closest('.settings-item-container');
        const batchVadSwitch = within(row as HTMLElement).getByRole('switch');
        expect(batchVadSwitch.getAttribute('aria-checked')).toBe('false');

        const restoreButton = screen.getByText('settings.restore_defaults');
        fireEvent.click(restoreButton);

        await waitFor(() => {
            expect(useConfigStore.getState().config.batchVadEnabled).toBe(true);
            expect(batchVadSwitch.getAttribute('aria-checked')).toBe('true');
        });
    });

    it('shows Off for both speaker dropdowns even when no speaker models are installed', async () => {
        renderTab(new Set());

        await waitFor(() => {
            expect(screen.getByRole('button', { name: 'Speaker Segmentation Model' }).textContent).toContain('Off');
            expect(screen.getByRole('button', { name: 'Speaker Embedding Model' }).textContent).toContain('Off');
        });

        // Expand accordions to mount ModelCards
        const segAccordion = await screen.findByRole('button', { name: /Speaker Segmentation Models/ });
        fireEvent.click(segAccordion);

        const embedAccordion = await screen.findByRole('button', { name: /Speaker Embedding Models/ });
        fireEvent.click(embedAccordion);

        expect(screen.getByTestId('model-card-sherpa-onnx-pyannote-segmentation-3-0').textContent).toContain('Pyannote 3.0');
        expect(screen.getByTestId('model-card-3dspeaker_speech_campplus_sv_zh_en_16k-common_advanced.onnx').textContent).toContain('3DSpeaker CAMPPlus');

        fireEvent.click(screen.getByRole('button', { name: 'Speaker Segmentation Model' }));
        expect(screen.getAllByRole('option').map((option) => option.textContent)).toEqual(['Off']);
    });

    it('keeps the page usable while local model catalog status is loading', async () => {
        setTestConfig({
            asr: {
                providers: {
                    online: {
                        'volcengine-doubao': {
                            apiKey: 'test-api-key',
                            streamingEndpoint: 'test-endpoint',
                            streamingResourceId: 'test-resource-id',
                            batchEndpoint: 'test-batch-endpoint',
                            batchResourceId: 'test-batch-resource',
                        },
                    },
                },
            },
            batchVadEnabled: true,
        } as any);

        renderTab(new Set(), {
            catalogLoadState: 'loading',
        });

        expect(screen.getByText('Checking local models...')).toBeDefined();
        expect((screen.getByRole('button', { name: 'settings.select_streaming_model' }) as HTMLButtonElement).disabled).toBe(true);
        expect((screen.getByLabelText('settings.restore_defaults') as HTMLButtonElement).disabled).toBe(true);
        expect(screen.queryByTestId('model-card-sherpa-onnx-pyannote-segmentation-3-0')).toBeNull();
        expect(screen.queryByRole('button', { name: /Speaker Segmentation Models/ })).toBeNull();
        expect(screen.queryByTestId('model-card-sherpa-onnx-pyannote-segmentation-3-0')).toBeNull();

        const apiKeyInput = screen.getByPlaceholderText('X-Api-Key') as HTMLInputElement;
        expect(apiKeyInput.disabled).toBe(false);

        const row = screen.getByText('settings.batch_vad_enabled').closest('.settings-item-container');
        const batchVadSwitch = within(row as HTMLElement).getByRole('switch');
        fireEvent.click(batchVadSwitch);

        await waitFor(() => {
            expect(useConfigStore.getState().config.batchVadEnabled).toBe(false);
        });
    });

    it('mounts local model cards and enables local actions after the catalog is ready', async () => {
        renderTab(new Set([
            'sherpa-onnx-pyannote-segmentation-3-0',
            '3dspeaker_speech_campplus_sv_zh_en_16k-common_advanced.onnx',
        ]));

        expect((screen.getByRole('button', { name: 'settings.select_streaming_model' }) as HTMLButtonElement).disabled).toBe(false);
        expect((screen.getByLabelText('settings.restore_defaults') as HTMLButtonElement).disabled).toBe(false);
        expect(screen.queryByTestId('model-card-sherpa-onnx-pyannote-segmentation-3-0')).toBeNull();
        expect(screen.queryByRole('button', { name: /Speaker Segmentation Models/ })).toBeNull();

        await waitFor(() => {
            expect(screen.getByRole('button', { name: /Speaker Segmentation Models/ })).toBeDefined();
        });
        fireEvent.click(screen.getByRole('button', { name: /Speaker Segmentation Models/ }));
        expect(screen.getByTestId('model-card-sherpa-onnx-pyannote-segmentation-3-0').textContent).toContain('Pyannote 3.0');
        expect((within(screen.getByTestId('model-card-sherpa-onnx-pyannote-segmentation-3-0')).getByRole('button', { name: 'model-action' }) as HTMLButtonElement).disabled).toBe(false);
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
        setTestConfig({
            asr: {
                providers: {
                    online: {
                        'volcengine-doubao': {
                            apiKey: 'test-api-key',
                            streamingEndpoint: 'test-endpoint',
                            streamingResourceId: 'test-resource-id',
                            batchEndpoint: 'test-batch-endpoint',
                            batchResourceId: 'test-batch-resource',
                        }
                    }
                }
            }
        } as any);

        renderTab(new Set());

        fireEvent.click(screen.getByRole('button', { name: 'settings.select_streaming_model' }));
        fireEvent.click(screen.getByRole('option', { name: '豆包语音 (火山)' }));

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
                providers: {
                    online: {
                        'volcengine-doubao': {
                            apiKey: 'test',
                            streamingEndpoint: 'test',
                            streamingResourceId: 'test',
                            batchEndpoint: 'test',
                            batchResourceId: 'test',
                        }
                    }
                },
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
            expect(screen.getByRole('button', { name: '豆包语音 (火山)' })).not.toBeNull();
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
