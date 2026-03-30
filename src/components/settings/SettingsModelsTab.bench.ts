import { bench, describe } from 'vitest';

// Simulating the PRESET_MODELS array with a few items to represent the models
const MOCK_MODELS = Array.from({ length: 20 }, (_, i) => ({
    id: `model-${i}`,
    modes: ['streaming', 'offline']
}));

const TARGET_MODEL_ID = 'model-19'; // The last model so sequential search takes the longest

// Simulate Tauri IPC call with ~5ms delay
const getModelPath = async (id: string): Promise<string> => {
    return new Promise(resolve => {
        setTimeout(() => {
            resolve(`/mock/models/path/${id}`);
        }, 5);
    });
};

describe('SettingsModelsTab sync model ID by path', () => {
    bench('sequential for...of loop (baseline)', async () => {
        const streamingModelPath = `/mock/models/path/${TARGET_MODEL_ID}`;
        let selectedId = '';

        for (const model of MOCK_MODELS) {
            if (model.modes?.includes('streaming')) {
                const path = await getModelPath(model.id);
                if (path === streamingModelPath) {
                    selectedId = model.id;
                    break;
                }
            }
        }

        // Ensure variable is used to satisfy TypeScript
        if (selectedId !== TARGET_MODEL_ID) {
            throw new Error('Test failed: Incorrect model ID');
        }
    });

    bench('concurrent Promise.all (optimized)', async () => {
        const streamingModelPath = `/mock/models/path/${TARGET_MODEL_ID}`;
        let selectedId = '';

        const streamingModels = MOCK_MODELS.filter(m => m.modes?.includes('streaming'));
        const paths = await Promise.all(streamingModels.map(m => getModelPath(m.id)));

        const index = paths.findIndex(path => path === streamingModelPath);
        if (index !== -1) {
            selectedId = streamingModels[index].id;
        }

        // Ensure variable is used to satisfy TypeScript
        if (selectedId !== TARGET_MODEL_ID) {
            throw new Error('Test failed: Incorrect model ID');
        }
    });
});