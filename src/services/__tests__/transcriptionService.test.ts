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
        subscribe: vi.fn(() => () => undefined),
    },
}));

vi.mock('../modelService', () => ({
    PRESET_MODELS: [],
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
    return module.TranscriptionService;
}

async function syncTranscriptConfig() {
    const { useTranscriptStore } = await import('../../stores/transcriptStore');
    useTranscriptStore.setState((state: any) => ({
        config: {
            ...state.config,
            ...mocks.config,
        },
    }));
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
        };
        mocks.invoke.mockImplementation(async () => undefined);
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
                id: 'seg-1',
                text: '测试123',
                isFinal: false,
            })
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
                segmentId: 'seg-1',
                processedTextLength: 5,
                callbackInvoked: true,
            })
        );
    });

    it('logs when text replacements collapse a voice-typing segment to empty text', async () => {
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
                id: 'seg-2',
                text: '',
            })
        );
        expect(mocks.loggerInfo).toHaveBeenCalledWith(
            '[TranscriptionService:voice-typing] Prepared recognizer segment for callback',
            expect.objectContaining({
                instanceId: 'voice-typing',
                segmentId: 'seg-2',
                rawTextLength: 5,
                processedTextLength: 0,
                replacementChanged: true,
                replacedToEmpty: true,
            })
        );
    });
});
