import { act, renderHook } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import type { HistoryItem } from '../../../../types/history';
import type { ContextMenuOpenRequest } from '../../../context-menu/trigger';
import type { OpenContextMenuOptions } from '../../../context-menu/types';
import { useHistoryItemContextMenu } from '../useHistoryItemContextMenu';

const dummyItem1: HistoryItem = {
  id: 'item-1',
  title: 'First Item',
  timestamp: 1000,
  duration: 60,
  audioPath: 'item-1.wav',
  transcriptPath: 'item-1.json',
  previewText: 'Hello world',
  searchContent: 'Hello world',
  type: 'recording',
  projectId: 'proj-1',
};

const dummyItem2: HistoryItem = {
  id: 'item-2',
  title: 'Second Item',
  timestamp: 2000,
  duration: 120,
  audioPath: 'item-2.wav',
  transcriptPath: 'item-2.json',
  previewText: 'Another transcript',
  searchContent: 'Another transcript',
  type: 'recording',
  projectId: null,
};

const dummyTrashItem: HistoryItem = {
  id: 'item-trash',
  title: 'Trash Item',
  timestamp: 3000,
  duration: 30,
  audioPath: 'item-trash.wav',
  transcriptPath: 'item-trash.json',
  previewText: 'Deleted text',
  searchContent: 'Deleted text',
  type: 'recording',
  projectId: null,
  deletedAt: 4000,
};

function createDummyRequest(): ContextMenuOpenRequest {
  return {
    anchor: document.createElement('button'),
    point: { x: 200, y: 300 },
    invocation: 'pointer',
  };
}

const t = ((key: string, options?: { defaultValue?: string }) =>
  options?.defaultValue ?? key) as any;

describe('useHistoryItemContextMenu', () => {
  it('opens single item normal menu with open, copy, tags, move to inbox, rename, reveal, and delete', () => {
    let capturedOptions: OpenContextMenuOptions | null = null;
    const openContextMenu = vi.fn((opts) => {
      capturedOptions = opts;
    });
    const onOpenItem = vi.fn();
    const onCopyTranscript = vi.fn();
    const onCopyTitle = vi.fn();
    const onAssignProject = vi.fn();
    const onMoveToInbox = vi.fn();
    const onRenameHistoryItem = vi.fn();
    const onShowInFolder = vi.fn();
    const onDeleteHistoryItem = vi.fn();

    const { result } = renderHook(() =>
      useHistoryItemContextMenu({
        getItemById: (id) => (id === 'item-1' ? dummyItem1 : undefined),
        isAllSelected: false,
        isLockedLiveDraft: () => false,
        isOpenDisabled: () => false,
        isTrashScope: false,
        onAssignProject,
        onClearSelection: vi.fn(),
        onCopyTitle,
        onCopyTranscript,
        onDeleteHistoryItem,
        onDeleteHistoryItems: vi.fn(),
        onMoveToInbox,
        onOpenItem,
        onRenameHistoryItem,
        onRestoreHistoryItems: vi.fn(),
        onSelectAllVisible: vi.fn(),
        onShowInFolder,
        openContextMenu,
        selectedIds: [],
        t,
      })
    );

    act(() => {
      result.current.openHistoryContextMenu('item-1', createDummyRequest());
    });

    expect(openContextMenu).toHaveBeenCalledTimes(1);
    expect(capturedOptions).not.toBeNull();
    const actionIds = capturedOptions!.actions.map((action) => action.id);
    expect(actionIds).toEqual([
      'open',
      'copy_transcript',
      'copy_title',
      'tags',
      'move_to_inbox',
      'rename',
      'reveal_file',
      'delete',
    ]);

    // Test actions invocation
    capturedOptions!.actions.find((action) => action.id === 'open')?.onSelect();
    expect(onOpenItem).toHaveBeenCalledWith(dummyItem1);

    capturedOptions!.actions.find((action) => action.id === 'copy_transcript')?.onSelect();
    expect(onCopyTranscript).toHaveBeenCalledWith(dummyItem1);

    capturedOptions!.actions.find((action) => action.id === 'copy_title')?.onSelect();
    expect(onCopyTitle).toHaveBeenCalledWith(dummyItem1);

    capturedOptions!.actions.find((action) => action.id === 'tags')?.onSelect();
    expect(onAssignProject).toHaveBeenCalledWith(['item-1']);

    capturedOptions!.actions.find((action) => action.id === 'move_to_inbox')?.onSelect();
    expect(onMoveToInbox).toHaveBeenCalledWith(['item-1']);

    capturedOptions!.actions.find((action) => action.id === 'rename')?.onSelect();
    expect(onRenameHistoryItem).toHaveBeenCalledWith('item-1');

    capturedOptions!.actions.find((action) => action.id === 'reveal_file')?.onSelect();
    expect(onShowInFolder).toHaveBeenCalledWith(dummyItem1);

    const deleteAction = capturedOptions!.actions.find((action) => action.id === 'delete');
    expect(deleteAction?.tone).toBe('danger');
    deleteAction?.onSelect();
    expect(onDeleteHistoryItem).toHaveBeenCalledWith('item-1');
  });

  it('opens single item trash menu with restore and purge', () => {
    let capturedOptions: OpenContextMenuOptions | null = null;
    const openContextMenu = vi.fn((opts) => {
      capturedOptions = opts;
    });
    const onRestoreHistoryItems = vi.fn();
    const onDeleteHistoryItem = vi.fn();

    const { result } = renderHook(() =>
      useHistoryItemContextMenu({
        getItemById: (id) => (id === 'item-trash' ? dummyTrashItem : undefined),
        isAllSelected: false,
        isLockedLiveDraft: () => false,
        isOpenDisabled: () => false,
        isTrashScope: true,
        onAssignProject: vi.fn(),
        onClearSelection: vi.fn(),
        onDeleteHistoryItem,
        onDeleteHistoryItems: vi.fn(),
        onMoveToInbox: vi.fn(),
        onOpenItem: vi.fn(),
        onRenameHistoryItem: vi.fn(),
        onRestoreHistoryItems,
        onSelectAllVisible: vi.fn(),
        openContextMenu,
        selectedIds: [],
        t,
      })
    );

    act(() => {
      result.current.openHistoryContextMenu('item-trash', createDummyRequest());
    });

    expect(openContextMenu).toHaveBeenCalledTimes(1);
    const actionIds = capturedOptions!.actions.map((action) => action.id);
    expect(actionIds).toEqual(['restore', 'purge']);

    capturedOptions!.actions.find((action) => action.id === 'restore')?.onSelect();
    expect(onRestoreHistoryItems).toHaveBeenCalledWith(['item-trash']);

    const purgeAction = capturedOptions!.actions.find((action) => action.id === 'purge');
    expect(purgeAction?.tone).toBe('danger');
    purgeAction?.onSelect();
    expect(onDeleteHistoryItem).toHaveBeenCalledWith('item-trash');
  });

  it('opens batch context menu when right-clicking a selected item in multi-select', () => {
    let capturedOptions: OpenContextMenuOptions | null = null;
    const openContextMenu = vi.fn((opts) => {
      capturedOptions = opts;
    });
    const onAssignProject = vi.fn();
    const onMoveToInbox = vi.fn();
    const onCopyTranscripts = vi.fn();
    const onDeleteHistoryItems = vi.fn();

    const items = [dummyItem1, dummyItem2];
    const { result } = renderHook(() =>
      useHistoryItemContextMenu({
        getItemById: (id) => items.find((item) => item.id === id),
        isAllSelected: false,
        isLockedLiveDraft: () => false,
        isOpenDisabled: () => false,
        isTrashScope: false,
        onAssignProject,
        onClearSelection: vi.fn(),
        onCopyTranscripts,
        onDeleteHistoryItem: vi.fn(),
        onDeleteHistoryItems,
        onMoveToInbox,
        onOpenItem: vi.fn(),
        onRenameHistoryItem: vi.fn(),
        onRestoreHistoryItems: vi.fn(),
        onSelectAllVisible: vi.fn(),
        openContextMenu,
        selectedIds: ['item-1', 'item-2'],
        t,
      })
    );

    act(() => {
      result.current.openHistoryContextMenu('item-1', createDummyRequest());
    });

    expect(openContextMenu).toHaveBeenCalledTimes(1);
    expect(capturedOptions!.contextId).toBe('workspace:history:batch:2');

    const actionIds = capturedOptions!.actions.map((action) => action.id);
    expect(actionIds).toEqual([
      'assign_project_batch',
      'move_to_inbox_batch',
      'copy_transcripts_batch',
      'select_all_visible',
      'delete_selected',
    ]);

    capturedOptions!.actions.find((action) => action.id === 'assign_project_batch')?.onSelect();
    expect(onAssignProject).toHaveBeenCalledWith(['item-1', 'item-2']);

    capturedOptions!.actions.find((action) => action.id === 'move_to_inbox_batch')?.onSelect();
    expect(onMoveToInbox).toHaveBeenCalledWith(['item-1', 'item-2']);

    capturedOptions!.actions.find((action) => action.id === 'copy_transcripts_batch')?.onSelect();
    expect(onCopyTranscripts).toHaveBeenCalledWith(items);

    const deleteBatchAction = capturedOptions!.actions.find(
      (action) => action.id === 'delete_selected'
    );
    expect(deleteBatchAction?.tone).toBe('danger');
    deleteBatchAction?.onSelect();
    expect(onDeleteHistoryItems).toHaveBeenCalledWith(['item-1', 'item-2']);
  });

  it('opens batch trash context menu when right-clicking selected items in trash', () => {
    let capturedOptions: OpenContextMenuOptions | null = null;
    const openContextMenu = vi.fn((opts) => {
      capturedOptions = opts;
    });
    const onRestoreHistoryItems = vi.fn();
    const onDeleteHistoryItems = vi.fn();

    const { result } = renderHook(() =>
      useHistoryItemContextMenu({
        getItemById: () => dummyTrashItem,
        isAllSelected: true,
        isLockedLiveDraft: () => false,
        isOpenDisabled: () => false,
        isTrashScope: true,
        onAssignProject: vi.fn(),
        onClearSelection: vi.fn(),
        onDeleteHistoryItem: vi.fn(),
        onDeleteHistoryItems,
        onMoveToInbox: vi.fn(),
        onOpenItem: vi.fn(),
        onRenameHistoryItem: vi.fn(),
        onRestoreHistoryItems,
        onSelectAllVisible: vi.fn(),
        openContextMenu,
        selectedIds: ['item-trash', 'item-trash-2'],
        t,
      })
    );

    act(() => {
      result.current.openHistoryContextMenu('item-trash', createDummyRequest());
    });

    expect(openContextMenu).toHaveBeenCalledTimes(1);
    const actionIds = capturedOptions!.actions.map((action) => action.id);
    expect(actionIds).toEqual(['restore_selected', 'clear_selection', 'purge_selected']);

    capturedOptions!.actions.find((action) => action.id === 'restore_selected')?.onSelect();
    expect(onRestoreHistoryItems).toHaveBeenCalledWith(['item-trash', 'item-trash-2']);

    const purgeSelectedAction = capturedOptions!.actions.find(
      (action) => action.id === 'purge_selected'
    );
    expect(purgeSelectedAction?.tone).toBe('danger');
    purgeSelectedAction?.onSelect();
    expect(onDeleteHistoryItems).toHaveBeenCalledWith(['item-trash', 'item-trash-2']);
  });

  it('clears previous selection when right-clicking an unselected item', () => {
    const onClearSelection = vi.fn();
    const openContextMenu = vi.fn();

    const { result } = renderHook(() =>
      useHistoryItemContextMenu({
        getItemById: (id) => (id === 'item-2' ? dummyItem2 : dummyItem1),
        isAllSelected: false,
        isLockedLiveDraft: () => false,
        isOpenDisabled: () => false,
        isTrashScope: false,
        onAssignProject: vi.fn(),
        onClearSelection,
        onDeleteHistoryItem: vi.fn(),
        onDeleteHistoryItems: vi.fn(),
        onMoveToInbox: vi.fn(),
        onOpenItem: vi.fn(),
        onRenameHistoryItem: vi.fn(),
        onRestoreHistoryItems: vi.fn(),
        onSelectAllVisible: vi.fn(),
        openContextMenu,
        selectedIds: ['item-1'], // item-1 is selected, but user right-clicks item-2
        t,
      })
    );

    act(() => {
      result.current.openHistoryContextMenu('item-2', createDummyRequest());
    });

    expect(onClearSelection).toHaveBeenCalledTimes(1);
    expect(openContextMenu).toHaveBeenCalledTimes(1);
  });
});
