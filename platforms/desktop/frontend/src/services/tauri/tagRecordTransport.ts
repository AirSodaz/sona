import type { TagRecord as GeneratedTagRecord } from '../../bindings';
import type { TagRecord } from '../../types/tag';

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
