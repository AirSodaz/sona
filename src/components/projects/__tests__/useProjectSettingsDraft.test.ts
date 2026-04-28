import { act, renderHook } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useProjectSettingsDraft } from '../hooks/useProjectSettingsDraft';
import type { ProjectRecord } from '../../../types/project';

const projectAlpha: ProjectRecord = {
  id: 'project-1',
  name: 'Alpha',
  description: 'Alpha project',
  icon: '🧪',
  createdAt: 1,
  updatedAt: 1,
  defaults: {
    summaryTemplateId: 'general',
    translationLanguage: 'zh',
    polishPresetId: 'general',
    exportFileNamePrefix: '',
    enabledTextReplacementSetIds: [],
    enabledHotwordSetIds: [],
    enabledPolishKeywordSetIds: [],
    enabledSpeakerProfileIds: [],
  },
};

const projectBeta: ProjectRecord = {
  ...projectAlpha,
  id: 'project-2',
  name: 'Beta',
  icon: '📁',
};

describe('useProjectSettingsDraft', () => {
  const t = (key: string, options?: Record<string, unknown>) => (
    typeof options?.defaultValue === 'string' ? options.defaultValue : key
  );

  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('hydrates icon state from the active project and rehydrates after project switches', async () => {
    const confirm = vi.fn().mockResolvedValue(true);
    const { result, rerender } = renderHook(
      ({ browseProject }) => useProjectSettingsDraft({ browseProject, confirm, t }),
      {
        initialProps: { browseProject: projectAlpha as ProjectRecord | null },
      },
    );

    expect(result.current.draftIcon).toBe('🧪');

    await act(async () => {
      result.current.setDraftIcon('🎯');
      result.current.setIsSettingsOpen(true);
    });

    expect(result.current.isProjectSettingsDirty).toBe(true);

    rerender({ browseProject: projectBeta });
    expect(result.current.draftIcon).toBe('📁');
    expect(result.current.isProjectSettingsDirty).toBe(false);
  });

  it('confirms discard only when settings are open and the draft is dirty', async () => {
    const confirm = vi.fn().mockResolvedValue(true);
    const { result } = renderHook(() => useProjectSettingsDraft({
      browseProject: projectAlpha,
      confirm,
      t,
    }));

    await act(async () => {
      result.current.setIsSettingsOpen(true);
      result.current.setDraftIcon('🎯');
    });

    await act(async () => {
      await result.current.handleRequestCloseProjectSettings();
    });

    expect(confirm).toHaveBeenCalledTimes(1);
    expect(result.current.isSettingsOpen).toBe(false);
    expect(result.current.draftIcon).toBe('🧪');
  });
});
