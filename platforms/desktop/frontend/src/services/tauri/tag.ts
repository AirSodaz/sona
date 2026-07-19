import type { TagRecord } from '../../types/tag';
import { TauriCommand } from './commands';
import type { TauriCommandArgs } from './contracts';
import { invokeTauri } from './invoke';
import { normalizeTagRecord, toTagRecordTransport } from './tagRecordTransport';

export type TagListRequest = TauriCommandArgs<typeof TauriCommand.tag.list>;
export type TagCreateRequest = TauriCommandArgs<typeof TauriCommand.tag.create>;
export type TagUpdateRequest = TauriCommandArgs<typeof TauriCommand.tag.update>['updates'];

export async function tagList(request: TagListRequest = {}): Promise<TagRecord[]> {
  const tags = await invokeTauri(TauriCommand.tag.list, request);
  return tags.map(normalizeTagRecord);
}

export async function tagSaveAll(tags: TagRecord[]): Promise<void> {
  await invokeTauri(TauriCommand.tag.saveAll, {
    tags: tags.map(toTagRecordTransport),
  });
}

export async function tagCreate(request: TagCreateRequest): Promise<TagRecord> {
  const tag = await invokeTauri(TauriCommand.tag.create, request);
  return normalizeTagRecord(tag);
}

export async function tagUpdate(
  tagId: string,
  updates: TagUpdateRequest,
): Promise<TagRecord | null> {
  const tag = await invokeTauri(TauriCommand.tag.update, { tagId, updates });
  return tag ? normalizeTagRecord(tag) : null;
}

export async function tagDelete(tagId: string): Promise<void> {
  await invokeTauri(TauriCommand.tag.delete, { tagId });
}

export async function tagReorder(tagIds: string[]): Promise<TagRecord[]> {
  const tags = await invokeTauri(TauriCommand.tag.reorder, { tagIds });
  return tags.map(normalizeTagRecord);
}

export async function tagGetActiveId(): Promise<string | null> {
  return invokeTauri(TauriCommand.tag.getActiveId);
}

export async function tagSetActiveId(tagId: string | null): Promise<void> {
  await invokeTauri(TauriCommand.tag.setActiveId, { tagId });
}
