import { act, fireEvent, render, screen, within } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useHistoryStore } from '../../stores/historyStore';
import { useProjectStore } from '../../stores/projectStore';
import { DEFAULT_SESSION_DATA, useTranscriptStore } from '../../stores/transcriptStore';
import { resetTranscriptStores } from '../../test-utils/transcriptStoreTestUtils';
import { ProjectSelectorDropdown } from '../ProjectSelectorDropdown';

describe('ProjectSelectorDropdown', () => {
  beforeEach(() => {
    resetTranscriptStores();
    useProjectStore.setState({
      projects: [
        {
          id: 'proj-1',
          name: 'Alpha',
          description: '',
          color: '#3B82F6',
          createdAt: 1000,
          updatedAt: 1000,
        },
      ],
      activeProjectId: null,
      setActiveProjectId: vi.fn().mockResolvedValue(undefined),
      createProject: vi.fn().mockResolvedValue(null),
    });
    useHistoryStore.setState({
      items: [],
    });
  });

  it('stays locked during active recording even when active session has no sourceHistoryId', () => {
    useTranscriptStore.setState({
      isRecording: true,
      recordingSessionId: 'rec-draft-1',
      activeSessionId: 'default',
      sessions: {
        default: { ...DEFAULT_SESSION_DATA, sourceHistoryId: null },
        'rec-draft-1': { ...DEFAULT_SESSION_DATA, sourceHistoryId: 'rec-draft-1' },
      },
    });

    render(<ProjectSelectorDropdown onOpenProjects={vi.fn()} />);

    const trigger = screen.getByRole('button');
    expect((trigger as HTMLButtonElement).disabled).toBe(true);
  });

  it('closes an open popover when recording starts and displays accurate counts when idle', () => {
    useHistoryStore.setState({
      items: [
        {
          id: 'rec-draft-1',
          title: 'Live Draft',
          timestamp: 1000,
          duration: 5,
          audioPath: 'draft.wav',
          transcriptPath: 'draft.json',
          previewText: '',
          type: 'recording',
          projectId: null,
          status: 'draft',
          draftSource: 'live_record',
        },
        {
          id: 'completed-1',
          title: 'Completed',
          timestamp: 500,
          duration: 10,
          audioPath: 'c1.wav',
          transcriptPath: 'c1.json',
          previewText: 'Hello',
          type: 'recording',
          projectId: 'proj-1',
          status: 'complete',
        },
      ] as any,
    });
    useTranscriptStore.setState({
      isRecording: false,
      recordingSessionId: null,
    });

    render(<ProjectSelectorDropdown onOpenProjects={vi.fn()} />);

    fireEvent.click(screen.getByRole('button'));
    const listbox = screen.getByRole('listbox');
    expect(within(listbox).getByRole('button', { name: /Inbox/i }).textContent).toContain('1');
    expect(within(listbox).getByRole('option', { name: /Alpha/i }).textContent).toContain('1');

    act(() => {
      useTranscriptStore.setState({
        isRecording: true,
        recordingSessionId: 'rec-draft-1',
      });
    });

    expect(screen.queryByRole('listbox')).toBeNull();
    expect((screen.getByRole('button') as HTMLButtonElement).disabled).toBe(true);
  });
});
