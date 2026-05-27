import { beforeEach, describe, expect, it, vi } from 'vitest';

const mocks = vi.hoisted(() => {
    const listenCallbacks: Record<string, (event: any) => void> = {};

    return {
        listenCallbacks,
        invoke: vi.fn(async () => undefined),
        listen: vi.fn(async (eventName: string, callback: (event: any) => void) => {
            listenCallbacks[eventName] = callback;
            return () => {
                delete listenCallbacks[eventName];
            };
        }),
        loggerInfo: vi.fn(),
        loggerWarn: vi.fn(),
        loggerError: vi.fn(),
        loggerDebug: vi.fn(),
        config: {
            textReplacementSets: [],
            hotwordSets: [],
            punctuationModelPath: '',
            vadModelPath: '',
            vadBufferSize: 5.0,
            speakerSegmentationModelPath: '',
            speakerEmbeddingModelPath: '',
            speakerProfiles: [],
        } as Record<string, any>,
        getModelRules: vi.fn(() => ({
            requiresPunctuation: false,
            requiresVad: false,
        })),
    };
});

vi.mock('@tauri-apps/api/core', () => ({
    invoke: mocks.invoke,
}));

vi.mock('@tauri-apps/api/event', () => ({
    listen: mocks.listen,
}));

vi.mock('../../stores/configStore', () => ({
    DEFAULT_CONFIG: {
        textReplacementSets: [],
        hotwordSets: [],
        punctuationModelPath: '',
        vadModelPath: '',
        vadBufferSize: 5.0,
    },
    useConfigStore: {
        getState: vi.fn(() => ({
            config: mocks.config,
        })),
        setState: vi.fn((updater: any) => {
            const nextState = typeof updater === 'function'
                ? updater({ config: mocks.config })
                : updater;
            if (nextState?.config) {
                mocks.config = nextState.config;
            }
        }),
        subscribe: vi.fn(() => () => undefined),
    },
}));

vi.mock('../modelService', () => ({
    PRESET_MODELS: [],
    PRESET_MODELS_MAP: new Map(),
    modelService: {
        getModelRules: mocks.getModelRules,
    },
}));

vi.mock('../../utils/logger', () => ({
    logger: {
        info: mocks.loggerInfo,
        warn: mocks.loggerWarn,
        error: mocks.loggerError,
        debug: mocks.loggerDebug,
    },
}));

async function loadTranscriptionService() {
    const module = await import('../transcriptionService');
    const { getEffectiveConfigSnapshot } = await import('../../stores/effectiveConfigStore');
    const { initRecognizer, processBatchFile } = await import('../tauri/recognizer');

    class TestTranscriptionService extends module.TranscriptionService {
        constructor(instanceId: string) {
            super(instanceId, {
                getEffectiveConfigSnapshot,
                initRecognizer,
                processBatchFile,
            });
        }
    }
    return TestTranscriptionService;
}

async function syncTranscriptConfig() {
    const { useTranscriptStore } = await import('../../test-utils/transcriptStoreTestUtils');
    useTranscriptStore.setState((state: any) => ({
        config: {
            ...state.config,
            ...mocks.config,
        },
    }));
}

function getInvokePayload(commandName: string): Record<string, unknown> | undefined {
    const calls = mocks.invoke.mock.calls as unknown as Array<[string, unknown]>;
    const call = calls.find(([command]) => command === commandName);
    return call?.[1] as Record<string, unknown> | undefined;
}

describe('TranscriptionService voice typing diagnostics', () => {
    beforeEach(() => {
        vi.clearAllMocks();
        vi.resetModules();

        for (const key of Object.keys(mocks.listenCallbacks)) {
            delete mocks.listenCallbacks[key];
        }

        mocks.config = {
            textReplacementSets: [],
            hotwordSets: [],
            punctuationModelPath: '',
            vadModelPath: '',
            vadBufferSize: 5.0,
            speakerSegmentationModelPath: '',
            speakerEmbeddingModelPath: '',
            speakerProfiles: [],
        };
        mocks.invoke.mockImplementation(async () => undefined);
    });

    it('rejects when streaming ASR is not configured', async () => {
        const TranscriptionService = await loadTranscriptionService();
        await syncTranscriptConfig();
        const service = new TranscriptionService('record');
        const onUpdate = vi.fn();
        const onError = vi.fn();

        await expect(service.start(onUpdate, onError)).rejects.toThrow('ASR is not configured');

        expect(onError).toHaveBeenCalledWith('ASR is not configured');
        expect(mocks.invoke).not.toHaveBeenCalledWith('start_recognizer', expect.anything());
    });

    it('logs raw and processed voice-typing text before invoking the callback', async () => {
        const TranscriptionService = await loadTranscriptionService();
        await syncTranscriptConfig();
        const service = new TranscriptionService('voice-typing');
        const onSegment = vi.fn();
        const onError = vi.fn();

        service.setModelPath('path/to/model');
        await service.start(onSegment, onError);

        mocks.listenCallbacks['recognizer-output-voice-typing']?.({
            payload: {
                id: 'seg-1',
                start: 0,
                end: 0.5,
                text: '测试123',
                isFinal: false,
            },
        });

        expect(onSegment).toHaveBeenCalledWith(
            expect.objectContaining({
                removeIds: [],
                upsertSegments: [
                    expect.objectContaining({
                        id: 'seg-1',
                        text: '测试123',
                        isFinal: false,
                    }),
                ],
            }),
        );
        expect(mocks.loggerInfo).toHaveBeenCalledWith(
            '[TranscriptionService:voice-typing] Prepared recognizer segment for callback',
            expect.objectContaining({
                instanceId: 'voice-typing',
                segmentId: 'seg-1',
                rawTextLength: 5,
                processedTextLength: 5,
                callbackInvoked: false,
                preview: '测试123',
            })
        );
        expect(mocks.loggerInfo).toHaveBeenCalledWith(
            '[TranscriptionService:voice-typing] Invoking callback',
            expect.objectContaining({
                instanceId: 'voice-typing',
                segmentIds: ['seg-1'],
                processedTextLength: 5,
                callbackInvoked: true,
            })
        );
        expect(mocks.invoke).toHaveBeenCalledWith('init_recognizer', expect.objectContaining({
            asrRequest: expect.objectContaining({
                postprocessOptions: {
                    textReplacementSets: [],
                    dropFinalDotSegments: true,
                },
            }),
        }));
        const initPayload = getInvokePayload('init_recognizer');
        expect(initPayload).not.toHaveProperty('postprocessOptions');
        expect(initPayload).not.toHaveProperty('language');
    });

    it('does not apply frontend text replacements to recognizer events', async () => {
        mocks.config = {
            ...mocks.config,
            textReplacementSets: [
                {
                    id: 'set-1',
                    name: 'test',
                    enabled: true,
                    ignoreCase: false,
                    rules: [
                        {
                            from: '测试123',
                            to: '',
                        },
                    ],
                },
            ],
        };

        const TranscriptionService = await loadTranscriptionService();
        await syncTranscriptConfig();
        const service = new TranscriptionService('voice-typing');
        const onSegment = vi.fn();
        const onError = vi.fn();

        service.setModelPath('path/to/model');
        await service.start(onSegment, onError);

        mocks.listenCallbacks['recognizer-output-voice-typing']?.({
            payload: {
                id: 'seg-2',
                start: 0,
                end: 0.5,
                text: '测试123',
                isFinal: false,
            },
        });

        expect(onSegment).toHaveBeenCalledWith(
            expect.objectContaining({
                removeIds: [],
                upsertSegments: [
                    expect.objectContaining({
                        id: 'seg-2',
                        text: '测试123',
                    }),
                ],
            }),
        );
        expect(mocks.loggerInfo).toHaveBeenCalledWith(
            '[TranscriptionService:voice-typing] Prepared recognizer segment for callback',
            expect.objectContaining({
                instanceId: 'voice-typing',
                segmentId: 'seg-2',
                rawTextLength: 5,
                processedTextLength: 5,
            })
        );
        expect(mocks.invoke).toHaveBeenCalledWith('init_recognizer', expect.objectContaining({
            asrRequest: expect.objectContaining({
                postprocessOptions: {
                    textReplacementSets: mocks.config.textReplacementSets,
                    dropFinalDotSegments: true,
                },
            }),
        }));
    });

    it('sends null speakerProcessing for batch transcription when either speaker model is off', async () => {
        mocks.config = {
            ...mocks.config,
            speakerSegmentationModelPath: '/models/speaker-segmentation',
            speakerEmbeddingModelPath: '',
            speakerProfiles: [
                { id: 'profile-1', name: 'Alice', enabled: true, samples: [] },
            ],
        };
        mocks.invoke.mockImplementation((async (...args: any[]) => {
            const [command] = args;
            if (command === 'process_batch_file') {
                return [];
            }
            return undefined;
        }) as any);

        const TranscriptionService = await loadTranscriptionService();
        await syncTranscriptConfig();
        const service = new TranscriptionService('record');
        service.setModelPath('/models/offline');

        await service.transcribeFile('C:/audio/demo.wav');

        expect(mocks.invoke).toHaveBeenCalledWith('process_batch_file', expect.objectContaining({
            speakerProcessing: null,
        }));
    });

    it('sends speakerProcessing for batch transcription when both speaker models are configured', async () => {
        mocks.config = {
            ...mocks.config,
            speakerSegmentationModelPath: '/models/speaker-segmentation',
            speakerEmbeddingModelPath: '/models/speaker-embedding.onnx',
            speakerProfiles: [
                { id: 'profile-1', name: 'Alice', enabled: true, samples: [] },
            ],
        };
        mocks.invoke.mockImplementation((async (...args: any[]) => {
            const [command] = args;
            if (command === 'process_batch_file') {
                return [];
            }
            return undefined;
        }) as any);

        const TranscriptionService = await loadTranscriptionService();
        await syncTranscriptConfig();
        const service = new TranscriptionService('record');
        service.setModelPath('/models/offline');

        await service.transcribeFile('C:/audio/demo.wav');

        expect(mocks.invoke).toHaveBeenCalledWith('process_batch_file', expect.objectContaining({
            speakerProcessing: {
                speakerSegmentationModelPath: '/models/speaker-segmentation',
                speakerEmbeddingModelPath: '/models/speaker-embedding.onnx',
                speakerProfiles: [
                    { id: 'profile-1', name: 'Alice', enabled: true, samples: [] },
                ],
            },
        }));
    });

    it('passes postprocess options to batch and returns backend segments without local filtering', async () => {
        mocks.config = {
            ...mocks.config,
            textReplacementSets: [
                {
                    id: 'set-1',
                    name: 'test',
                    enabled: true,
                    ignoreCase: false,
                    rules: [
                        {
                            from: 'apple',
                            to: 'orange',
                        },
                    ],
                },
            ],
        };
        const backendSegments = [
            {
                id: 'seg-apple',
                text: 'apple',
                start: 0,
                end: 1,
                isFinal: true,
            },
            {
                id: 'seg-dot',
                text: '.',
                start: 1,
                end: 1.2,
                isFinal: true,
            },
        ];
        mocks.invoke.mockImplementation((async (...args: any[]) => {
            const [command] = args;
            if (command === 'process_batch_file') {
                return backendSegments;
            }
            return undefined;
        }) as any);

        const TranscriptionService = await loadTranscriptionService();
        await syncTranscriptConfig();
        const service = new TranscriptionService('record');
        service.setModelPath('/models/offline');

        const segments = await service.transcribeFile('C:/audio/demo.wav');

        expect(mocks.invoke).toHaveBeenCalledWith('process_batch_file', expect.objectContaining({
            asrRequest: expect.objectContaining({
                postprocessOptions: {
                    textReplacementSets: mocks.config.textReplacementSets,
                    dropFinalDotSegments: true,
                },
            }),
        }));
        const batchPayload = getInvokePayload('process_batch_file');
        expect(batchPayload).not.toHaveProperty('postprocessOptions');
        expect(batchPayload).not.toHaveProperty('language');
        expect(segments.map((segment) => ({ id: segment.id, text: segment.text }))).toEqual([
            { id: 'seg-apple', text: 'apple' },
            { id: 'seg-dot', text: '.' },
        ]);
    });

    it('starts an online Volcengine streaming recognizer without a local model path', async () => {
        mocks.config = {
            ...mocks.config,
            language: 'zh',
            enableITN: true,
            asr: {
                selections: {
                    live: {
                        engine: 'online',
                        mode: 'streaming',
                        modelId: null,
                        modelPath: '',
                        providerId: 'volcengine-doubao',
                        profileId: 'volcengine-doubao-default',
                    },
                    caption: {
                        engine: 'local-sherpa',
                        mode: 'streaming',
                        modelId: null,
                        modelPath: '',
                    },
                    voiceTyping: {
                        engine: 'local-sherpa',
                        mode: 'streaming',
                        modelId: null,
                        modelPath: '',
                    },
                    batch: {
                        engine: 'local-sherpa',
                        mode: 'offline',
                        modelId: null,
                        modelPath: '',
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
        };

        const TranscriptionService = await loadTranscriptionService();
        await syncTranscriptConfig();
        const service = new TranscriptionService('record');

        await service.start(vi.fn(), vi.fn());

        expect(mocks.invoke).toHaveBeenCalledWith('init_recognizer', expect.objectContaining({
            asrRequest: expect.objectContaining({
                engine: 'online',
                mode: 'streaming',
                modelPath: '',
                onlineProvider: expect.objectContaining({
                    providerId: 'volcengine-doubao',
                    profileId: 'volcengine-doubao-default',
                    config: expect.objectContaining({
                        apiKey: 'volc-test-key',
                        streamingResourceId: 'volc.seedasr.sauc.duration',
                    }),
                }),
            }),
        }));
        const initPayload = getInvokePayload('init_recognizer');
        expect(initPayload).not.toHaveProperty('modelPath');
        expect(initPayload).not.toHaveProperty('language');
        expect(mocks.invoke).toHaveBeenCalledWith('start_recognizer', { instanceId: 'record' });
    });

    it('transcribes an online Volcengine batch request without setting a local model path', async () => {
        mocks.config = {
            ...mocks.config,
            asr: {
                selections: {
                    live: {
                        engine: 'local-sherpa',
                        mode: 'streaming',
                        modelId: null,
                        modelPath: '',
                    },
                    caption: {
                        engine: 'local-sherpa',
                        mode: 'streaming',
                        modelId: null,
                        modelPath: '',
                    },
                    voiceTyping: {
                        engine: 'local-sherpa',
                        mode: 'streaming',
                        modelId: null,
                        modelPath: '',
                    },
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
        };
        mocks.invoke.mockImplementation((async (...args: any[]) => {
            const [command] = args;
            if (command === 'process_batch_file') {
                return [{
                    id: 'volc-1',
                    text: '关闭透传。',
                    start: 0.45,
                    end: 1.53,
                    isFinal: true,
                }];
            }
            return undefined;
        }) as any);

        const TranscriptionService = await loadTranscriptionService();
        await syncTranscriptConfig();
        const service = new TranscriptionService('record');

        const segments = await service.transcribeFile('C:/audio/demo.wav');

        expect(mocks.invoke).toHaveBeenCalledWith('process_batch_file', expect.objectContaining({
            asrRequest: expect.objectContaining({
                engine: 'online',
                mode: 'offline',
                modelPath: '',
                onlineProvider: expect.objectContaining({
                    providerId: 'volcengine-doubao',
                    config: expect.objectContaining({
                        apiKey: 'volc-test-key',
                        batchResourceId: 'volc.bigasr.auc_turbo',
                    }),
                }),
            }),
        }));
        const batchPayload = getInvokePayload('process_batch_file');
        expect(batchPayload).not.toHaveProperty('modelPath');
        expect(batchPayload).not.toHaveProperty('language');
        expect(segments).toEqual([
            expect.objectContaining({
                id: 'volc-1',
                text: '关闭透传。',
            }),
        ]);
    });

});
