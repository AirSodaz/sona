import type { AppConfig, AsrScenario } from '../types/config';

export type { AsrScenario };

export type ScenarioModelKind =
  | 'punctuationModelPath'
  | 'vadModelPath'
  | 'speakerSegmentationModelPath'
  | 'speakerEmbeddingModelPath';

type ScenarioPathField =
  | 'livePunctuationModelPath'
  | 'liveVadModelPath'
  | 'liveSpeakerSegmentationModelPath'
  | 'liveSpeakerEmbeddingModelPath'
  | 'batchPunctuationModelPath'
  | 'batchVadModelPath'
  | 'batchSpeakerSegmentationModelPath'
  | 'batchSpeakerEmbeddingModelPath';

/** Structural subset of AppConfig carrying the per-scenario model paths. */
export type ScenarioModelPathConfig = Pick<AppConfig, ScenarioPathField>;

type ScenarioModelFieldMap = Record<AsrScenario, Record<ScenarioModelKind, ScenarioPathField>>;

const SCENARIO_MODEL_FIELDS: ScenarioModelFieldMap = {
  live: {
    punctuationModelPath: 'livePunctuationModelPath',
    vadModelPath: 'liveVadModelPath',
    speakerSegmentationModelPath: 'liveSpeakerSegmentationModelPath',
    speakerEmbeddingModelPath: 'liveSpeakerEmbeddingModelPath',
  },
  batch: {
    punctuationModelPath: 'batchPunctuationModelPath',
    vadModelPath: 'batchVadModelPath',
    speakerSegmentationModelPath: 'batchSpeakerSegmentationModelPath',
    speakerEmbeddingModelPath: 'batchSpeakerEmbeddingModelPath',
  },
};

/** Resolves the concrete config field name for a scenario-scoped model kind. */
export function scenarioModelFieldKey(
  kind: ScenarioModelKind,
  scenario: AsrScenario,
): ScenarioPathField {
  return SCENARIO_MODEL_FIELDS[scenario][kind];
}

/** Reads a scenario-scoped model path from the config. */
export function getScenarioModelPath(
  config: ScenarioModelPathConfig,
  kind: ScenarioModelKind,
  scenario: AsrScenario,
): string {
  const key = scenarioModelFieldKey(kind, scenario);
  return config[key]?.trim() || '';
}

export function getScenarioPunctuationModelPath(config: ScenarioModelPathConfig, scenario: AsrScenario): string {
  return getScenarioModelPath(config, 'punctuationModelPath', scenario);
}

export function getScenarioVadModelPath(config: ScenarioModelPathConfig, scenario: AsrScenario): string {
  return getScenarioModelPath(config, 'vadModelPath', scenario);
}

export function getScenarioSpeakerSegmentationModelPath(
  config: ScenarioModelPathConfig,
  scenario: AsrScenario,
): string {
  return getScenarioModelPath(config, 'speakerSegmentationModelPath', scenario);
}

export function getScenarioSpeakerEmbeddingModelPath(
  config: ScenarioModelPathConfig,
  scenario: AsrScenario,
): string {
  return getScenarioModelPath(config, 'speakerEmbeddingModelPath', scenario);
}

export function getScenarioVadBufferSize(
  config: Pick<AppConfig, 'liveVadBufferSize' | 'batchVadBufferSize'>,
  scenario: AsrScenario,
): number {
  const value = scenario === 'batch' ? config.batchVadBufferSize : config.liveVadBufferSize;
  return Number.isFinite(value) && (value as number) > 0 ? (value as number) : 5;
}
