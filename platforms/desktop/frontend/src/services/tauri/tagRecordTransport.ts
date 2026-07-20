import type { TagRecord } from '../../types/tag';
import type { TagRecord as GeneratedTagRecord } from '../../bindings';

export function toTagRecordTransport(tag: TagRecord): GeneratedTagRecord {
  return {
    ...tag,
    color: tag.color ?? '#64748b',
    sortOrder: tag.sortOrder ?? 0,
  };
}

export function normalizeTagRecord(record: GeneratedTagRecord): TagRecord {
  return { ...record };
}
