import { describe, it, expect, vi, beforeEach } from 'vitest';
import { modelService, PRESET_MODELS } from '../modelService';
import { invoke } from '@tauri-apps/api/core';
import { exists, remove } from '@tauri-apps/plugin-fs';
import enLocale from '../../locales/en.json';
import zhLocale from '../../locales/zh.json';

// Mock mocks
vi.mock('@tauri-apps/api/core', () => ({
    invoke: vi.fn(),
}));

vi.mock('@tauri-apps/plugin-fs', () => ({
    exists: vi.fn(),
    mkdir: vi.fn(),
    remove: vi.fn(),
}));

vi.mock('@tauri-apps/api/path', () => ({
    appLocalDataDir: vi.fn().mockResolvedValue('/app/data'),
    join: vi.fn((...args) => Promise.resolve(args.join('/')))
}));

vi.mock('@tauri-apps/api/event', () => ({
    listen: vi.fn(),
}));

describe('ModelService', () => {
    beforeEach(() => {
        vi.clearAllMocks();
    });

    describe('checkHardware', () => {
        it('returns true for models', async () => {
            const modelId = PRESET_MODELS[0].id;
            const result = await modelService.checkHardware(modelId);
            expect(result.compatible).toBe(true);
            expect(invoke).not.toHaveBeenCalledWith('check_gpu_availability');
        });
    });

    describe('isModelInstalled', () => {
        it('returns true if file exists', async () => {
            // vi.mocked(join).mockResolvedValue('/app/data/models/test-model');
            (exists as any).mockResolvedValue(true);

            const result = await modelService.isModelInstalled('test-model');
            expect(result).toBe(true);
        });

        it('returns false if file does not exist', async () => {
            (exists as any).mockResolvedValue(false);
            const result = await modelService.isModelInstalled('test-model');
            expect(result).toBe(false);
        });
    });

    describe('downloadModel', () => {
        const modelId = PRESET_MODELS[0].id; // Use first available model

        it('downloads a model successfully', async () => {
            const onProgress = vi.fn();

            // Mock download_file invoke
            (invoke as any).mockImplementation((cmd: string) => {
                if (cmd === 'download_file') return Promise.resolve();
                return Promise.resolve();
            });


            await modelService.downloadModel(modelId, onProgress);

            expect(invoke).toHaveBeenCalledWith('download_file', expect.objectContaining({
                url: expect.stringContaining('http'),
                outputPath: expect.stringContaining('/app/data/models/'),
            }));
            expect(onProgress).toHaveBeenCalledWith(100, 'Done', true);
        });
    });

    describe('deleteModel', () => {
        it('removes the model directory/file if it exists', async () => {
            (exists as any).mockResolvedValue(true);
            (remove as any).mockResolvedValue(undefined);

            await modelService.deleteModel('test-model');

            expect(remove).toHaveBeenCalled();
        });
    });

    describe('ITN Models', () => {
        const itnModel = PRESET_MODELS.find(m => m.type === 'itn');

        it('checks if ITN model is installed', async () => {
            if (!itnModel) return; // Skip if no ITN model
            (exists as any).mockResolvedValue(true);
            const result = await modelService.isModelInstalled(itnModel.id);
            expect(result).toBe(true);
        });

        it('downloads ITN model', async () => {
            if (!itnModel) return;
            (exists as any).mockResolvedValue(false);
            (invoke as any).mockResolvedValue(undefined);
            await modelService.downloadModel(itnModel.id);

            expect(invoke).toHaveBeenCalledWith('download_file', expect.objectContaining({
                url: expect.stringContaining(itnModel.url)
            }));
        });
    });

    describe('Qwen3 ASR metadata', () => {
        const qwen3ModelId = 'sherpa-onnx-qwen3-asr-0.6B-int8-2026-03-25';

        it('registers qwen3-asr as an offline-only model with VAD support', () => {
            const qwen3Model = PRESET_MODELS.find((model) => model.id === qwen3ModelId);
            const offlineModelIds = PRESET_MODELS
                .filter((model) => model.modes?.includes('offline'))
                .map((model) => model.id);
            const streamingModelIds = PRESET_MODELS
                .filter((model) => model.modes?.includes('streaming'))
                .map((model) => model.id);

            expect(qwen3Model).toMatchObject({
                id: qwen3ModelId,
                type: 'qwen3-asr',
                modes: ['offline'],
                fileConfig: {
                    convFrontend: 'conv_frontend.onnx',
                    encoder: 'encoder.int8.onnx',
                    decoder: 'decoder.int8.onnx',
                    tokenizer: 'tokenizer',
                },
            });
            expect(qwen3Model?.fileConfig).not.toHaveProperty('tokens');
            expect(offlineModelIds).toContain(qwen3ModelId);
            expect(streamingModelIds).not.toContain(qwen3ModelId);
            expect(modelService.getModelRules(qwen3ModelId)).toEqual({
                requiresVad: true,
                requiresPunctuation: false,
            });
        });

        it('adds qwen3-asr description keys to both locale files', () => {
            expect(enLocale.settings.descriptions.qwen3_asr).toBeTruthy();
            expect(zhLocale.settings.descriptions.qwen3_asr).toBeTruthy();
        });
    });

    describe('speaker model metadata', () => {
        const segmentationIds = [
            'sherpa-onnx-pyannote-segmentation-3-0',
            'sherpa-onnx-reverb-diarization-v1',
            'sherpa-onnx-reverb-diarization-v2',
        ];
        const embeddingIds = [
            '3dspeaker_speech_campplus_sv_zh_en_16k-common_advanced.onnx',
            '3dspeaker_speech_eres2netv2_sv_zh-cn_16k-common.onnx',
            '3dspeaker_speech_eres2net_large_sv_zh-cn_3dspeaker_16k.onnx',
            '3dspeaker_speech_eres2net_sv_zh-cn_16k-common.onnx',
        ];

        it('registers the expected speaker segmentation archives and speaker embedding files', () => {
            const speakerSegmentationModels = PRESET_MODELS.filter((model) => model.type === 'speaker-segmentation');
            const speakerEmbeddingModels = PRESET_MODELS.filter((model) => model.type === 'speaker-embedding');

            expect(speakerSegmentationModels.map((model) => model.id)).toEqual(segmentationIds);
            expect(speakerEmbeddingModels.map((model) => model.id)).toEqual(embeddingIds);

            expect(speakerSegmentationModels).toEqual([
                expect.objectContaining({
                    id: 'sherpa-onnx-pyannote-segmentation-3-0',
                    name: 'Pyannote 3.0',
                    url: 'https://github.com/k2-fsa/sherpa-onnx/releases/download/speaker-segmentation-models/sherpa-onnx-pyannote-segmentation-3-0.tar.bz2',
                    type: 'speaker-segmentation',
                }),
                expect.objectContaining({
                    id: 'sherpa-onnx-reverb-diarization-v1',
                    name: 'Reverb Diarization V1',
                    url: 'https://github.com/k2-fsa/sherpa-onnx/releases/download/speaker-segmentation-models/sherpa-onnx-reverb-diarization-v1.tar.bz2',
                    type: 'speaker-segmentation',
                }),
                expect.objectContaining({
                    id: 'sherpa-onnx-reverb-diarization-v2',
                    name: 'Reverb Diarization V2',
                    url: 'https://github.com/k2-fsa/sherpa-onnx/releases/download/speaker-segmentation-models/sherpa-onnx-reverb-diarization-v2.tar.bz2',
                    type: 'speaker-segmentation',
                }),
            ]);

            expect(speakerEmbeddingModels).toEqual([
                expect.objectContaining({
                    id: '3dspeaker_speech_campplus_sv_zh_en_16k-common_advanced.onnx',
                    name: '3DSpeaker CAMPPlus',
                    isArchive: false,
                    filename: '3dspeaker_speech_campplus_sv_zh_en_16k-common_advanced.onnx',
                }),
                expect.objectContaining({
                    id: '3dspeaker_speech_eres2netv2_sv_zh-cn_16k-common.onnx',
                    name: '3DSpeaker ERes2NetV2',
                    isArchive: false,
                    filename: '3dspeaker_speech_eres2netv2_sv_zh-cn_16k-common.onnx',
                }),
                expect.objectContaining({
                    id: '3dspeaker_speech_eres2net_large_sv_zh-cn_3dspeaker_16k.onnx',
                    name: '3DSpeaker ERes2Net Large',
                    isArchive: false,
                    filename: '3dspeaker_speech_eres2net_large_sv_zh-cn_3dspeaker_16k.onnx',
                }),
                expect.objectContaining({
                    id: '3dspeaker_speech_eres2net_sv_zh-cn_16k-common.onnx',
                    name: '3DSpeaker ERes2Net',
                    isArchive: false,
                    filename: '3dspeaker_speech_eres2net_sv_zh-cn_16k-common.onnx',
                }),
            ]);
        });

        it('resolves archive speaker models to directories and file speaker models to filenames', async () => {
            await expect(modelService.getModelPath('sherpa-onnx-reverb-diarization-v1'))
                .resolves.toBe('/app/data/models/sherpa-onnx-reverb-diarization-v1');
            await expect(modelService.getModelPath('3dspeaker_speech_eres2netv2_sv_zh-cn_16k-common.onnx'))
                .resolves.toBe('/app/data/models/3dspeaker_speech_eres2netv2_sv_zh-cn_16k-common.onnx');
        });

        it('adds localized speaker settings labels to both locale files', () => {
            expect(enLocale.settings).toMatchObject({
                audio_files: 'Audio Files',
                speaker_segmentation_model_label: 'Speaker Segmentation Model',
                speaker_segmentation_model_hint: 'Used to split offline recordings into anonymous speaker turns.',
                select_speaker_segmentation_model: 'Select speaker segmentation model...',
                speaker_embedding_model_label: 'Speaker Embedding Model',
                speaker_embedding_model_hint: 'Used to match diarized speakers against your known speaker profiles.',
                select_speaker_embedding_model: 'Select speaker embedding model...',
                speaker_segmentation_models: 'Speaker Segmentation Models',
                speaker_embedding_models: 'Speaker Embedding Models',
                speaker_profiles_title: 'Speaker Profiles',
                speaker_profiles_description: 'Build a global library of known speakers from local reference audio files. Projects can then choose which profiles are active.',
                speaker_profile_name_label: 'Profile Name',
                speaker_profile_name_placeholder: 'e.g. Alice',
                add_speaker_profile: 'Add Profile',
                no_speaker_profiles: 'No speaker profiles yet.',
                speaker_profile_readiness_ready: 'Ready for automatic matching',
                speaker_profile_readiness_limited: 'Can appear as a suggestion, but needs more usable samples before automatic matching.',
                speaker_profile_readiness_not_ready: 'Needs more usable samples before it can participate in speaker recognition.',
                speaker_samples_count: '{{count}} samples',
                delete_speaker_profile: 'Delete {{name}}',
                speaker_profile_samples_hint: 'Import one or more local reference clips. They will be normalized to 16k mono WAV and stored under app-managed data.',
                speaker_profile_readiness_meta: '{{usable}} usable samples · {{duration}}',
                import_speaker_samples: 'Import Samples',
                no_speaker_samples: 'No reference samples imported yet.',
                speaker_profile_import_failed: 'Failed to import one or more speaker reference samples.',
            });

            expect(zhLocale.settings).toMatchObject({
                audio_files: '音频文件',
                speaker_segmentation_model_label: '说话人分离模型',
                speaker_segmentation_model_hint: '用于将离线录音拆分为匿名说话人片段。',
                select_speaker_segmentation_model: '选择说话人分离模型...',
                speaker_embedding_model_label: '说话人特征模型',
                speaker_embedding_model_hint: '用于将分离出的说话人与已知说话人档案匹配。',
                select_speaker_embedding_model: '选择说话人特征模型...',
                speaker_segmentation_models: '说话人分离模型',
                speaker_embedding_models: '说话人特征模型',
                speaker_profiles_title: '说话人档案',
                speaker_profiles_description: '从本地参考音频建立已知说话人档案库，项目可以选择启用哪些档案。',
                speaker_profile_name_label: '档案名称',
                speaker_profile_name_placeholder: '例如：Alice',
                add_speaker_profile: '添加档案',
                no_speaker_profiles: '还没有说话人档案。',
                speaker_profile_readiness_ready: '可用于自动匹配',
                speaker_profile_readiness_limited: '可以作为候选建议，但还需要更多可用样本才能自动匹配。',
                speaker_profile_readiness_not_ready: '需要更多可用样本后才能参与说话人识别。',
                speaker_samples_count: '{{count}} 个样本',
                delete_speaker_profile: '删除 {{name}}',
                speaker_profile_samples_hint: '导入一个或多个本地参考片段。它们会被规范化为 16k 单声道 WAV，并存放在应用管理的数据目录中。',
                speaker_profile_readiness_meta: '{{usable}} 个可用样本 · {{duration}}',
                import_speaker_samples: '导入样本',
                no_speaker_samples: '还没有导入参考样本。',
                speaker_profile_import_failed: '导入一个或多个说话人参考样本失败。',
            });
        });
    });
});
