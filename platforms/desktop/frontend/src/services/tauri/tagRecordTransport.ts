import type {
  TagDefaults_Deserialize,
  TagRecord_Deserialize,
  TagRecord_Serialize,
} from '../../bindings';
import { normalizeProjectRecord } from '../project/projectDefaults';
import type { TagDefaults, TagRecord } from '../../types/tag';

export function toTagDefaultsTransport(
  defaults: TagDefaults,
): TagDefaults_Deserialize {
  return {
    summaryTemplateId: defaults.summaryTemplateId,
    translationLanguage: defaults.translationLanguage,
    polishPresetId: defaults.polishPresetId,
    polishScenario: defaults.polishScenario ?? null,
    polishContext: defaults.polishContext ?? null,
    exportFileNamePrefix: defaults.exportFileNamePrefix,
    enabledTextReplacementSetIds: defaults.enabledTextReplacementSetIds,
    enabledHotwordSetIds: defaults.enabledHotwordSetIds,
    enabledPolishKeywordSetIds: defaults.enabledPolishKeywordSetIds,
    enabledSpeakerProfileIds: defaults.enabledSpeakerProfileIds,
  };
}

export function toTagRecordTransport(tag: TagRecord): TagRecord_Deserialize {
  return {
    id: tag.id,
    name: tag.name,
    description: tag.description,
    icon: tag.icon,
    color: tag.color ?? '#64748b',
    sortOrder: tag.sortOrder ?? 0,
    createdAt: tag.createdAt,
    updatedAt: tag.updatedAt,
    defaults: toTagDefaultsTransport(tag.defaults),
  };
}

export function normalizeTagRecord(record: TagRecord_Serialize): TagRecord {
  return normalizeProjectRecord(record);
}
