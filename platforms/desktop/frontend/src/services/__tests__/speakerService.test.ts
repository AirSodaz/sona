import { beforeEach, describe, expect, it, vi } from 'vitest';
import { speakerService } from '../speakerService';

vi.mock('@tauri-apps/api/core', () => ({
    invoke: vi.fn(),
}));

describe('speakerService', () => {
    beforeEach(() => {
        vi.clearAllMocks();
    });

    it('treats speaker processing as disabled when either model path is empty', () => {
        expect(speakerService.isConfigured({
            liveSpeakerSegmentationModelPath: '/models/seg',
            liveSpeakerEmbeddingModelPath: '',
            speakerProfiles: [],
        }, 'live')).toBe(false);

        expect(speakerService.buildProcessingConfig({
            liveSpeakerSegmentationModelPath: '',
            liveSpeakerEmbeddingModelPath: '/models/embed.onnx',
            speakerProfiles: [],
        }, 'live')).toBeNull();
    });

    it('builds speaker processing config only when both model paths are configured', () => {
        expect(speakerService.buildProcessingConfig({
            liveSpeakerSegmentationModelPath: '/models/seg',
            liveSpeakerEmbeddingModelPath: '/models/embed.onnx',
            speakerProfiles: [
                {
                    id: 'profile-1',
                    name: ' Alice ',
                    enabled: true,
                    samples: [
                        {
                            id: 'sample-1',
                            filePath: '/profiles/alice.wav',
                            sourceName: 'Alice WAV',
                            durationSeconds: 3.2,
                        },
                    ],
                },
            ],
        }, 'live')).toEqual({
            speakerSegmentationModelPath: '/models/seg',
            speakerEmbeddingModelPath: '/models/embed.onnx',
            speakerProfiles: [
                {
                    id: 'profile-1',
                    name: 'Alice',
                    enabled: true,
                    samples: [
                        {
                            id: 'sample-1',
                            filePath: '/profiles/alice.wav',
                            sourceName: 'Alice WAV',
                            durationSeconds: 3.2,
                        },
                    ],
                },
            ],
        });
    });

    it('resolves batch scenario paths independently from live paths', () => {
        expect(speakerService.buildProcessingConfig({
            liveSpeakerSegmentationModelPath: '',
            liveSpeakerEmbeddingModelPath: '',
            batchSpeakerSegmentationModelPath: '/models/batch-seg',
            batchSpeakerEmbeddingModelPath: '/models/batch-embed.onnx',
            speakerProfiles: [],
        }, 'batch')).toEqual({
            speakerSegmentationModelPath: '/models/batch-seg',
            speakerEmbeddingModelPath: '/models/batch-embed.onnx',
            speakerProfiles: [],
        });
    });
});
