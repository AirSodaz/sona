import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { ProjectsRail } from '../ProjectsRail';

describe('ProjectsRail', () => {
  const defaultProps = {
    activeContextId: null,
    browseProjectId: null,
    historyItemsCount: 10,
    inboxCount: 2,
    isAllItemsScope: true,
    isInboxScope: false,
    isTrashScope: false,
    trashCount: 1,
    itemCounts: new Map<string | null, number>([
      [null, 2],
      ['trash', 1],
      ['proj-1', 5],
    ]),
    onOpenCreateModal: vi.fn(),
    onReorderProjects: vi.fn().mockResolvedValue(undefined),
    onSwitchScope: vi.fn().mockResolvedValue(true),
    onOpenProjectContextMenu: vi.fn(),
    projects: [
      {
        id: 'proj-1',
        name: 'Test Project',
        description: 'Test description',
        color: '#3B82F6',
        createdAt: 1000,
        updatedAt: 1000,
      },
    ],
    t: (key: string, options?: { defaultValue?: string }) => options?.defaultValue ?? key,
  };

  it('renders section header with project group title and create button', () => {
    const { container } = render(<ProjectsRail {...defaultProps} />);

    // Redundant header is removed; scopes renders directly at the top
    expect(container.querySelector('.projects-rail-header')).toBeNull();
    const railScopes = container.querySelector('.projects-rail-scopes');
    expect(railScopes).not.toBeNull();
    expect(container.querySelector('.projects-rail')?.firstElementChild).toBe(railScopes);
    // Section header has section title and create button
    const sectionHeader = container.querySelector('.projects-rail-section-header');
    expect(sectionHeader).not.toBeNull();
    expect(sectionHeader?.querySelector('.projects-rail-section-title')?.textContent).toBe(
      'Projects'
    );

    const createButton = sectionHeader?.querySelector('.projects-rail-create');
    expect(createButton).not.toBeNull();
    expect(createButton?.getAttribute('aria-label')).toBe('New Project');
  });

  it('calls onOpenCreateModal when clicking the create button', () => {
    const onOpenCreateModal = vi.fn();
    render(<ProjectsRail {...defaultProps} onOpenCreateModal={onOpenCreateModal} />);

    const createButton = screen.getByRole('button', { name: 'New Project' });
    fireEvent.click(createButton);

    expect(onOpenCreateModal).toHaveBeenCalledTimes(1);
  });
});
