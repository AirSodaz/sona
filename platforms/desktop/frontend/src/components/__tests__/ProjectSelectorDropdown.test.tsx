import { act, fireEvent, render, screen, within } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useHistoryStore } from '../../stores/historyStore';
import { useProjectStore } from '../../stores/projectStore';
import { DEFAULT_SESSION_DATA, useTranscriptStore } from '../../stores/transcriptStore';
import { resetTranscriptStores } from '../../test-utils/transcriptStoreTestUtils';
import type { HistoryItem } from '../../types/history';
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

  it('stays enabled during active recording even when active session has no sourceHistoryId', () => {
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
    expect((trigger as HTMLButtonElement).disabled).toBe(false);
  });

  it('keeps popover open, preserves counts, and allows setActiveProjectId when recording starts', () => {
    const setActiveProjectIdMock = vi.fn().mockResolvedValue(undefined);
    useProjectStore.setState({
      setActiveProjectId: setActiveProjectIdMock,
    });
    const items: HistoryItem[] = [
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
    ];
    useHistoryStore.setState({ items });
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

    const openListbox = screen.getByRole('listbox');
    expect((screen.getByRole('button', { expanded: true }) as HTMLButtonElement).disabled).toBe(
      false
    );
    fireEvent.click(within(openListbox).getByRole('option', { name: /Alpha/i }));
    expect(setActiveProjectIdMock).toHaveBeenCalledWith('proj-1');
  });

  it('calls setActiveProjectId when creating a project from the modal even after recording starts', async () => {
    const createdProject = {
      id: 'proj-created',
      name: 'Beta',
      description: '',
      color: '#64748b',
      createdAt: 2000,
      updatedAt: 2000,
    };
    const createProjectMock = vi.fn().mockResolvedValue(createdProject);
    const setActiveProjectIdMock = vi.fn().mockResolvedValue(undefined);
    useProjectStore.setState({
      createProject: createProjectMock,
      setActiveProjectId: setActiveProjectIdMock,
    });

    render(<ProjectSelectorDropdown onOpenProjects={vi.fn()} />);

    fireEvent.click(screen.getByRole('button'));
    fireEvent.click(screen.getByRole('button', { name: /New Project/i }));

    // Dropdown popover should be closed after clicking New Project
    expect(screen.queryByRole('listbox')).toBeNull();

    act(() => {
      useTranscriptStore.setState({
        isRecording: true,
        recordingSessionId: 'rec-draft-1',
      });
    });

    const nameInput = screen.getByLabelText(/Project name/i);
    fireEvent.change(nameInput, { target: { value: 'Beta' } });

    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: /Create Tag/i }));
    });

    expect(createProjectMock).toHaveBeenCalledTimes(1);
    expect(setActiveProjectIdMock).toHaveBeenCalledWith('proj-created');
  });

  it('closes dropdown popover and triggers onOpenProjects when clicking Open Project Center', () => {
    const onOpenProjectsMock = vi.fn();
    render(<ProjectSelectorDropdown onOpenProjects={onOpenProjectsMock} />);

    // Open dropdown
    fireEvent.click(screen.getByRole('button'));
    expect(screen.getByRole('listbox')).toBeTruthy();

    // Click Open Project Center
    fireEvent.click(screen.getByRole('button', { name: /Open Project Center/i }));

    expect(onOpenProjectsMock).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole('listbox')).toBeNull();
  });

  it('toggles chevron rotation state when opening and closing the dropdown', () => {
    render(<ProjectSelectorDropdown onOpenProjects={vi.fn()} />);

    const trigger = screen.getByRole('button');
    const chevron = trigger.querySelector('.project-selector-chevron');
    expect(chevron).not.toBeNull();
    expect(trigger.getAttribute('aria-expanded')).toBe('false');
    expect(chevron?.classList.contains('is-open')).toBe(false);

    fireEvent.click(trigger);
    expect(trigger.getAttribute('aria-expanded')).toBe('true');
    expect(chevron?.classList.contains('is-open')).toBe(true);

    fireEvent.click(trigger);
    expect(trigger.getAttribute('aria-expanded')).toBe('false');
    expect(chevron?.classList.contains('is-open')).toBe(false);
  });
});
