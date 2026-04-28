import { PRESET_MODELS, type ModelInfo } from '../services/modelService';

function normalizeModelPath(modelPath: string): string {
    return modelPath.replace(/\\/g, '/').toLowerCase();
}

function getModelPathToken(model: Pick<ModelInfo, 'filename' | 'id'>): string {
    return (model.filename || model.id).replace(/\\/g, '/').toLowerCase();
}

export function doesModelPathMatch(
    modelPath: string,
    model: Pick<ModelInfo, 'filename' | 'id'>,
): boolean {
    if (!modelPath.trim()) {
        return false;
    }

    return normalizeModelPath(modelPath).includes(getModelPathToken(model));
}

export function findSelectedModelByMode(
    modelPath: string,
    mode: 'streaming' | 'offline',
): ModelInfo | null {
    return (
        PRESET_MODELS.find(
            (model) => model.modes?.includes(mode) && doesModelPathMatch(modelPath, model),
        ) ?? null
    );
}

export function findSelectedModelByType(
    modelPath: string,
    type: ModelInfo['type'],
): ModelInfo | null {
    return (
        PRESET_MODELS.find(
            (model) => model.type === type && doesModelPathMatch(modelPath, model),
        ) ?? null
    );
}
