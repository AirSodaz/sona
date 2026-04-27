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
            speakerSegmentationModelPath: '/models/seg',
            speakerEmbeddingModelPath: '',
            speakerProfiles: [],
        })).toBe(false);

        expect(speakerService.buildProcessingConfig({
            speakerSegmentationModelPath: '',
            speakerEmbeddingModelPath: '/models/embed.onnx',
            speakerProfiles: [],
        })).toBeNull();
    });

    it('builds speaker processing config only when both model paths are configured', () => {
        expect(speakerService.buildProcessingConfig({
            speakerSegmentationModelPath: '/models/seg',
            speakerEmbeddingModelPath: '/models/embed.onnx',
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
        })).toEqual({
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
});
