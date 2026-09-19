import { beforeEach, describe, expect, it, vi } from 'vitest';
import { speakerService } from '../speakerService';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

describe('speakerService', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('treats speaker processing as disabled when required model paths are missing', () => {
    expect(
      speakerService.isConfigured(
        {
          liveSpeakerSegmentationModelPath: '/models/seg',
          liveSpeakerEmbeddingModelPath: '',
          speakerProfiles: [],
        },
        'live'
      )
    ).toBe(false);

    expect(
      speakerService.isConfigured(
        {
          batchSpeakerSegmentationModelPath: '',
          batchSpeakerEmbeddingModelPath: '/models/embed.onnx',
          speakerProfiles: [],
        },
        'batch'
      )
    ).toBe(false);

    expect(
      speakerService.buildProcessingConfig(
        {
          batchSpeakerSegmentationModelPath: '',
          batchSpeakerEmbeddingModelPath: '/models/embed.onnx',
          speakerProfiles: [],
        },
        'batch'
      )
    ).toBeNull();
  });

  it('builds speaker processing config only when both model paths are configured', () => {
    expect(
      speakerService.buildProcessingConfig(
        {
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
        },
        'live'
      )
    ).toEqual({
      speakerSegmentationModelPath: '/models/seg',
      speakerEmbeddingModelPath: '/models/embed.onnx',
      speakerProfiles: [
        {
          id: 'profile-1',
          name: 'Alice',
          enabled: true,
          scope: 'global',
          projectIds: [],
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
      sensitivity: 'balanced',
    });
  });

  it('resolves batch scenario paths independently from live paths', () => {
    expect(
      speakerService.buildProcessingConfig(
        {
          liveSpeakerSegmentationModelPath: '',
          liveSpeakerEmbeddingModelPath: '',
          batchSpeakerSegmentationModelPath: '/models/batch-seg',
          batchSpeakerEmbeddingModelPath: '/models/batch-embed.onnx',
          speakerProfiles: [],
        },
        'batch'
      )
    ).toEqual({
      speakerSegmentationModelPath: '/models/batch-seg',
      speakerEmbeddingModelPath: '/models/batch-embed.onnx',
      speakerProfiles: [],
      sensitivity: 'balanced',
    });
  });

  it('skips file annotation when segmentation model is missing', async () => {
    const sampleSegments = [{ id: 'seg-1', start: 0, end: 1, text: 'hello' } as any];
    const result = await speakerService.annotateSegmentsForFile(
      '/path/to/audio.wav',
      sampleSegments,
      {
        liveSpeakerSegmentationModelPath: '',
        liveSpeakerEmbeddingModelPath: '/models/embed.onnx',
        speakerProfiles: [],
      },
      'live'
    );
    expect(result).toBe(sampleSegments);
  });

  it('resolves effective profiles based on project scope and enablement', () => {
    const profiles = [
      {
        id: 'global-1',
        name: 'Alice',
        enabled: true,
        scope: 'global' as const,
        projectIds: [],
        samples: [],
      },
      {
        id: 'global-disabled',
        name: 'Bob',
        enabled: false,
        scope: 'global' as const,
        projectIds: [],
        samples: [],
      },
      {
        id: 'proj-a-only',
        name: 'Charlie',
        enabled: true,
        scope: 'project' as const,
        projectIds: ['project-a'],
        samples: [],
      },
      {
        id: 'multi-proj',
        name: 'David',
        enabled: true,
        scope: 'project' as const,
        projectIds: ['project-a', 'project-b'],
        samples: [],
      },
      {
        id: 'proj-c-only',
        name: 'Eve',
        enabled: true,
        scope: 'project' as const,
        projectIds: ['project-c'],
        samples: [],
      },
      {
        id: 'proj-disabled',
        name: 'Frank',
        enabled: false,
        scope: 'project' as const,
        projectIds: ['project-a'],
        samples: [],
      },
    ];

    // For Project A: Alice (global), Charlie (A), David (A & B)
    const effectiveA = speakerService.resolveEffectiveProfiles(profiles, 'project-a');
    expect(effectiveA.map((p) => p.id)).toEqual(['global-1', 'proj-a-only', 'multi-proj']);

    // For Project B: Alice (global), David (A & B)
    const effectiveB = speakerService.resolveEffectiveProfiles(profiles, 'project-b');
    expect(effectiveB.map((p) => p.id)).toEqual(['global-1', 'multi-proj']);

    // For Project C: Alice (global), Eve (C)
    const effectiveC = speakerService.resolveEffectiveProfiles(profiles, 'project-c');
    expect(effectiveC.map((p) => p.id)).toEqual(['global-1', 'proj-c-only']);

    // For Inbox / No Project: Alice (global) only
    const effectiveNone = speakerService.resolveEffectiveProfiles(profiles, null);
    expect(effectiveNone.map((p) => p.id)).toEqual(['global-1']);
  });
});
