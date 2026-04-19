import { AppConfig } from '../types/transcript';
import { ModelInfo, PRESET_MODELS, modelService } from './modelService';

export const RECOMMENDED_RECOGNITION_MODEL_ID =
  'sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17';
export const RECOMMENDED_VAD_MODEL_ID = 'silero-vad';

export interface OnboardingDownloadUpdate {
  modelId: string;
  percentage: number;
  status: string;
  isFinished?: boolean;
}

export interface RecommendedOnboardingPaths {
  streamingModelPath: string;
  offlineModelPath: string;
  vadModelPath: string;
}

/**
 * Returns the recommended model set used by the first-run wizard.
 */
export function getRecommendedOnboardingModels(): ModelInfo[] {
  return PRESET_MODELS.filter((model) => (
    model.id === RECOMMENDED_RECOGNITION_MODEL_ID || model.id === RECOMMENDED_VAD_MODEL_ID
  )).sort((left, right) => {
    const order = [RECOMMENDED_RECOGNITION_MODEL_ID, RECOMMENDED_VAD_MODEL_ID];
    return order.indexOf(left.id) - order.indexOf(right.id);
  });
}

/**
 * Resolves filesystem paths for the recommended onboarding models.
 */
export async function resolveRecommendedOnboardingPaths(): Promise<RecommendedOnboardingPaths> {
  const recognitionPath = await modelService.getModelPath(RECOMMENDED_RECOGNITION_MODEL_ID);
  const vadPath = await modelService.getModelPath(RECOMMENDED_VAD_MODEL_ID);

  return {
    streamingModelPath: recognitionPath,
    offlineModelPath: recognitionPath,
    vadModelPath: vadPath,
  };
}

/**
 * Builds the config fragment applied after onboarding downloads complete.
 */
export function getRecommendedOnboardingConfig(
  paths: RecommendedOnboardingPaths,
): Partial<AppConfig> {
  return {
    streamingModelPath: paths.streamingModelPath,
    offlineModelPath: paths.offlineModelPath,
    vadModelPath: paths.vadModelPath,
    enableITN: true,
  };
}

/**
 * Downloads the recommended onboarding models and reports per-model progress.
 */
export async function downloadRecommendedOnboardingModels(
  onUpdate?: (update: OnboardingDownloadUpdate) => void,
  signal?: AbortSignal,
): Promise<RecommendedOnboardingPaths> {
  const downloads = await Promise.all(
    getRecommendedOnboardingModels().map(async (model) => {
      const path = await modelService.downloadModel(
        model.id,
        (percentage, status, isFinished) => {
          onUpdate?.({
            modelId: model.id,
            percentage,
            status,
            isFinished,
          });
        },
        signal,
      );

      return {
        modelId: model.id,
        path,
      };
    }),
  );

  const recognitionPath = downloads.find((item) => item.modelId === RECOMMENDED_RECOGNITION_MODEL_ID)?.path;
  const vadPath = downloads.find((item) => item.modelId === RECOMMENDED_VAD_MODEL_ID)?.path;

  if (!recognitionPath || !vadPath) {
    throw new Error('Failed to resolve recommended onboarding models');
  }

  return {
    streamingModelPath: recognitionPath,
    offlineModelPath: recognitionPath,
    vadModelPath: vadPath,
  };
}
