import { act, renderHook } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import type { ProjectRecord } from '../../../../types/project';
import type { OpenContextMenuOptions } from '../../../context-menu/types';
import type { ContextMenuOpenRequest } from '../../../context-menu/trigger';
import { ALL_ITEMS_SCOPE, TRASH_SCOPE, UNTAGGED_SCOPE } from '../../constants';
import { useProjectRailContextMenu } from '../useProjectRailContextMenu';

const dummyProject: ProjectRecord = {
  id: 'proj-1',
  name: 'Test Project',
  description: 'Test description',
  icon: '📁',
  createdAt: 100,
  updatedAt: 200,
};

function createDummyRequest(): ContextMenuOpenRequest {
  return {
    anchor: document.createElement('button'),
    point: { x: 100, y: 150 },
    invocation: 'pointer',
  };
}

const t = ((key: string, options?: { defaultValue?: string }) => options?.defaultValue ?? key) as any;

describe('useProjectRailContextMenu', () => {
  it('opens custom project menu with open, set active, settings, and delete actions', () => {
    let capturedOptions: OpenContextMenuOptions | null = null;
    const openContextMenu = vi.fn((opts) => {
      capturedOptions = opts;
    });
    const onSwitchScope = vi.fn().mockResolvedValue(true);
    const onSetActiveProjectId = vi.fn();
    const onOpenProjectSettings = vi.fn();
    const onDeleteProject = vi.fn();

    const { result } = renderHook(() => useProjectRailContextMenu({
      activeProjectId: null,
      browseScope: 'all',
      isLockedLiveDraft: false,
      onDeleteProject,
      onOpenProjectSettings,
      onSetActiveProjectId,
      onSwitchScope,
      openContextMenu,
      projects: [dummyProject],
      t,
    }));

    act(() => {
      result.current.openRailContextMenu('proj-1', createDummyRequest());
    });

    expect(openContextMenu).toHaveBeenCalledTimes(1);
    expect(capturedOptions).not.toBeNull();
    const actionIds = capturedOptions!.actions.map((action) => action.id);
    expect(actionIds).toEqual(['open', 'set_active', 'settings', 'delete']);

    // Check action execution
    const openAction = capturedOptions!.actions.find((action) => action.id === 'open');
    openAction?.onSelect();
    expect(onSwitchScope).toHaveBeenCalledWith('proj-1');

    const setActiveAction = capturedOptions!.actions.find((action) => action.id === 'set_active');
    setActiveAction?.onSelect();
    expect(onSetActiveProjectId).toHaveBeenCalledWith('proj-1');

    const settingsAction = capturedOptions!.actions.find((action) => action.id === 'settings');
    settingsAction?.onSelect();
    expect(onOpenProjectSettings).toHaveBeenCalledWith('proj-1');

    const deleteAction = capturedOptions!.actions.find((action) => action.id === 'delete');
    deleteAction?.onSelect();
    expect(onDeleteProject).toHaveBeenCalledWith(dummyProject);
    expect(deleteAction?.tone).toBe('danger');
  });

  it('opens inbox context menu with open and set active actions', () => {
    let capturedOptions: OpenContextMenuOptions | null = null;
    const openContextMenu = vi.fn((opts) => {
      capturedOptions = opts;
    });
    const onSwitchScope = vi.fn().mockResolvedValue(true);
    const onSetActiveProjectId = vi.fn();

    const { result } = renderHook(() => useProjectRailContextMenu({
      activeProjectId: 'proj-1',
      browseScope: 'all',
      isLockedLiveDraft: false,
      onDeleteProject: vi.fn(),
      onOpenProjectSettings: vi.fn(),
      onSetActiveProjectId,
      onSwitchScope,
      openContextMenu,
      projects: [dummyProject],
      t,
    }));

    act(() => {
      result.current.openRailContextMenu(UNTAGGED_SCOPE, createDummyRequest());
    });

    expect(openContextMenu).toHaveBeenCalledTimes(1);
    const actionIds = capturedOptions!.actions.map((action) => action.id);
    expect(actionIds).toEqual(['open_inbox', 'set_active_inbox']);

    const setActiveInbox = capturedOptions!.actions.find((action) => action.id === 'set_active_inbox');
    setActiveInbox?.onSelect();
    expect(onSetActiveProjectId).toHaveBeenCalledWith(null);
  });

  it('opens trash context menu and triggers empty trash', () => {
    let capturedOptions: OpenContextMenuOptions | null = null;
    const openContextMenu = vi.fn((opts) => {
      capturedOptions = opts;
    });
    const onSwitchScope = vi.fn().mockResolvedValue(true);
    const onEmptyTrash = vi.fn();

    const { result } = renderHook(() => useProjectRailContextMenu({
      activeProjectId: null,
      browseScope: 'all',
      isLockedLiveDraft: false,
      onDeleteProject: vi.fn(),
      onEmptyTrash,
      onOpenProjectSettings: vi.fn(),
      onSetActiveProjectId: vi.fn(),
      onSwitchScope,
      openContextMenu,
      projects: [dummyProject],
      t,
      trashCount: 5,
    }));

    act(() => {
      result.current.openRailContextMenu(TRASH_SCOPE, createDummyRequest());
    });

    expect(openContextMenu).toHaveBeenCalledTimes(1);
    const actionIds = capturedOptions!.actions.map((action) => action.id);
    expect(actionIds).toEqual(['open_trash', 'empty_trash']);

    const emptyTrashAction = capturedOptions!.actions.find((action) => action.id === 'empty_trash');
    expect(emptyTrashAction?.disabled).toBe(false);
    expect(emptyTrashAction?.tone).toBe('danger');
    emptyTrashAction?.onSelect();
    expect(onEmptyTrash).toHaveBeenCalledTimes(1);
  });

  it('opens all items context menu', () => {
    let capturedOptions: OpenContextMenuOptions | null = null;
    const openContextMenu = vi.fn((opts) => {
      capturedOptions = opts;
    });
    const onSwitchScope = vi.fn().mockResolvedValue(true);

    const { result } = renderHook(() => useProjectRailContextMenu({
      activeProjectId: null,
      browseScope: 'proj-1',
      isLockedLiveDraft: false,
      onDeleteProject: vi.fn(),
      onOpenProjectSettings: vi.fn(),
      onSetActiveProjectId: vi.fn(),
      onSwitchScope,
      openContextMenu,
      projects: [dummyProject],
      t,
    }));

    act(() => {
      result.current.openRailContextMenu(ALL_ITEMS_SCOPE, createDummyRequest());
    });

    expect(openContextMenu).toHaveBeenCalledTimes(1);
    const actionIds = capturedOptions!.actions.map((action) => action.id);
    expect(actionIds).toEqual(['open_all']);
  });

  it('opens rail empty area menu to create new project', () => {
    let capturedOptions: OpenContextMenuOptions | null = null;
    const openContextMenu = vi.fn((opts) => {
      capturedOptions = opts;
    });
    const onOpenCreateModal = vi.fn();

    const { result } = renderHook(() => useProjectRailContextMenu({
      activeProjectId: null,
      browseScope: 'all',
      isLockedLiveDraft: false,
      onDeleteProject: vi.fn(),
      onOpenCreateModal,
      onOpenProjectSettings: vi.fn(),
      onSetActiveProjectId: vi.fn(),
      onSwitchScope: vi.fn().mockResolvedValue(true),
      openContextMenu,
      projects: [dummyProject],
      t,
    }));

    act(() => {
      result.current.openRailContextMenu('rail_empty', createDummyRequest());
    });

    expect(openContextMenu).toHaveBeenCalledTimes(1);
    const actionIds = capturedOptions!.actions.map((action) => action.id);
    expect(actionIds).toEqual(['new_project']);

    const newProjectAction = capturedOptions!.actions.find((action) => action.id === 'new_project');
    newProjectAction?.onSelect();
    expect(onOpenCreateModal).toHaveBeenCalledTimes(1);
  });
});
