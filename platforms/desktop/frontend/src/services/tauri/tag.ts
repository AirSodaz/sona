import type { TagRecord } from '../../types/tag';
import { TauriCommand } from './commands';
import type { TauriCommandArgs } from './contracts';
import { invokeTauri } from './invoke';

export type TagListRequest = TauriCommandArgs<typeof TauriCommand.tag.list>;
export type TagCreateRequest = TauriCommandArgs<typeof TauriCommand.tag.create>;
export type TagUpdateRequest = TauriCommandArgs<typeof TauriCommand.tag.update>['updates'];

export async function tagList(request: TagListRequest = {}): Promise<TagRecord[]> {
  return invokeTauri(TauriCommand.tag.list, request) as Promise<TagRecord[]>;
}

export async function tagSaveAll(tags: TagRecord[]): Promise<void> {
  await invokeTauri(TauriCommand.tag.saveAll, { tags });
}

export async function tagCreate(request: TagCreateRequest): Promise<TagRecord> {
  return invokeTauri(TauriCommand.tag.create, request) as Promise<TagRecord>;
}

export async function tagUpdate(
  tagId: string,
  updates: TagUpdateRequest,
): Promise<TagRecord | null> {
  return invokeTauri(TauriCommand.tag.update, { tagId, updates }) as Promise<TagRecord | null>;
}

export async function tagDelete(tagId: string): Promise<void> {
  await invokeTauri(TauriCommand.tag.delete, { tagId });
}

export async function tagReorder(tagIds: string[]): Promise<TagRecord[]> {
  return invokeTauri(TauriCommand.tag.reorder, { tagIds }) as Promise<TagRecord[]>;
}

export async function tagGetActiveId(): Promise<string | null> {
  return invokeTauri(TauriCommand.tag.getActiveId);
}

export async function tagSetActiveId(tagId: string | null): Promise<void> {
  await invokeTauri(TauriCommand.tag.setActiveId, { tagId });
}
