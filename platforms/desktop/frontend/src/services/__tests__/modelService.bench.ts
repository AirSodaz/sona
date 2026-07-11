import { bench, describe } from 'vitest';
import { PRESET_MODELS, modelService, DEFAULT_MODEL_RULES } from '../modelService';

describe('ModelService benchmark', () => {
    bench('getModelRules - original', () => {
        const getModelRulesOriginal = (modelId: string) => {
            const model = PRESET_MODELS.find(m => m.id === modelId);
            if (model && model.rules) {
                return model.rules;
            }
            return DEFAULT_MODEL_RULES;
        };
        getModelRulesOriginal('sherpa-onnx-paraformer-zh-2023-09-14');
        getModelRulesOriginal('sherpa-onnx-sensevoice-zh-en-ja-ko-yue-2024-07-17');
        getModelRulesOriginal('non-existent-model');
    });

    bench('getModelRules - optimized Map (current implementation)', () => {
        modelService.getModelRules('sherpa-onnx-paraformer-zh-2023-09-14');
        modelService.getModelRules('sherpa-onnx-sensevoice-zh-en-ja-ko-yue-2024-07-17');
        modelService.getModelRules('non-existent-model');
    });
});
