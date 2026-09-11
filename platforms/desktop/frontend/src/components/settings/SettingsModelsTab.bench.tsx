import { test } from 'vitest';

// Mock model type
interface ModelInfo {
    id: string;
    modes?: string[];
}

// Generate some mock models
const mockModels: ModelInfo[] = Array.from({ length: 20 }, (_, i) => ({
    id: `model-${i}`,
    modes: i % 2 === 0 ? ['streaming'] : ['batch', 'streaming']
}));

// Mock model path resolution - simulate a small delay to mimic IPC call
const getModelPath = async (id: string): Promise<string> => {
    // We add a tiny artificial delay to simulate the IPC boundary
    await new Promise(resolve => setTimeout(resolve, 1));
    return `/path/to/models/${id}`;
};

test('SettingsModelsTab model resolution', async ({ bench }) => {
    await bench('baseline sequential loop (streaming + batch)', async () => {
        const streamingModelPath = '/path/to/models/model-18';
        const batchModelPath = '/path/to/models/model-19';

        let selectedStreamingModelId = '';
        let selectedBatchModelId = '';

        // Streaming logic
        if (streamingModelPath) {
            for (const model of mockModels) {
                if (model.modes?.includes('streaming')) {
                    const path = await getModelPath(model.id);
                    if (path === streamingModelPath) {
                        selectedStreamingModelId = model.id;
                        break;
                    }
                }
            }
        }

        // Offline logic
        if (batchModelPath) {
            for (const model of mockModels) {
                if (model.modes?.includes('batch')) {
                    const path = await getModelPath(model.id);
                    if (path === batchModelPath) {
                        selectedBatchModelId = model.id;
                        break;
                    }
                }
            }
        }

        // Use variables to prevent strict compiler errors
        if (!selectedStreamingModelId || !selectedBatchModelId) {
            return;
        }
    }).run();

    await bench('optimized map with Promise.all', async () => {
        const streamingModelPath = '/path/to/models/model-18';
        const batchModelPath = '/path/to/models/model-19';

        let selectedStreamingModelId = '';
        let selectedBatchModelId = '';

        // Optimization logic: map path -> id
        const pathRecord: Record<string, string> = {};

        await Promise.all(
            mockModels.map(async (model) => {
                const path = await getModelPath(model.id);
                pathRecord[path] = model.id;
            })
        );

        if (streamingModelPath) {
            selectedStreamingModelId = pathRecord[streamingModelPath] || '';
        }

        if (batchModelPath) {
            selectedBatchModelId = pathRecord[batchModelPath] || '';
        }

        // Use variables to prevent strict compiler errors
        if (!selectedStreamingModelId || !selectedBatchModelId) {
            return;
        }
    }).run();
});
