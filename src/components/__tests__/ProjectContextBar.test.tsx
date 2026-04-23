import { fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { ProjectContextBar } from '../ProjectContextBar';

const mockSetMode = vi.fn();
const mockProjectState = {
  activeProjectId: 'project-1' as string | null,
  projects: [
    {
      id: 'project-1',
      name: 'Alpha',
    },
  ],
};

vi.mock('../../stores/projectStore', () => ({
  useProjectStore: (selector: any) => selector(mockProjectState),
}));

vi.mock('../../stores/transcriptStore', () => ({
  useTranscriptStore: (selector: any) => selector({ setMode: mockSetMode }),
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, options?: any) => options?.defaultValue || key,
  }),
  initReactI18next: {
    type: '3rdParty',
    init: () => undefined,
  },
}));

describe('ProjectContextBar', () => {
  beforeEach(() => {
    mockSetMode.mockReset();
    mockProjectState.activeProjectId = 'project-1';
  });

  it('shows the active project and opens Projects mode', () => {
    render(<ProjectContextBar />);

    expect(screen.getByText('Current Project')).toBeDefined();
    expect(screen.getByText('Alpha')).toBeDefined();

    fireEvent.click(screen.getByRole('button', { name: 'Open Workspace' }));
    expect(mockSetMode).toHaveBeenCalledWith('projects');
  });

  it('renders nothing without an active project', () => {
    mockProjectState.activeProjectId = null;

    const { container } = render(<ProjectContextBar />);
    expect(container.firstChild).toBeNull();
  });
});
