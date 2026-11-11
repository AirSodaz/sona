import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { ProjectsRail } from '../ProjectsRail';

describe('ProjectsRail', () => {
  const defaultProps = {
    activeProjectId: null,
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

  it('calls onOpenProjectContextMenu with keyboard request when menu button activated by keyboard', () => {
    const onOpenProjectContextMenu = vi.fn();
    render(<ProjectsRail {...defaultProps} onOpenProjectContextMenu={onOpenProjectContextMenu} />);

    const menuBtns = screen.getAllByRole('button', { name: 'More options' });
    fireEvent.click(menuBtns[1], { detail: 0, clientX: 0, clientY: 0 });

    expect(onOpenProjectContextMenu).toHaveBeenCalledTimes(1);
    expect(onOpenProjectContextMenu).toHaveBeenCalledWith(
      'proj-1',
      expect.objectContaining({ invocation: 'keyboard' })
    );
  });

  it('calls onOpenProjectContextMenu with pointer request when menu button activated by mouse', () => {
    const onOpenProjectContextMenu = vi.fn();
    render(<ProjectsRail {...defaultProps} onOpenProjectContextMenu={onOpenProjectContextMenu} />);

    const menuBtns = screen.getAllByRole('button', { name: 'More options' });
    fireEvent.click(menuBtns[1], { detail: 1, clientX: 150, clientY: 200 });

    expect(onOpenProjectContextMenu).toHaveBeenCalledTimes(1);
    expect(onOpenProjectContextMenu).toHaveBeenCalledWith(
      'proj-1',
      expect.objectContaining({ invocation: 'pointer', point: { x: 150, y: 200 } })
    );
  });

  it('calls onOpenProjectContextMenu when menu button receives contextmenu or context menu key', () => {
    const onOpenProjectContextMenu = vi.fn();
    render(<ProjectsRail {...defaultProps} onOpenProjectContextMenu={onOpenProjectContextMenu} />);

    const menuBtns = screen.getAllByRole('button', { name: 'More options' });
    fireEvent.contextMenu(menuBtns[1]);
    expect(onOpenProjectContextMenu).toHaveBeenCalledWith('proj-1', expect.anything());

    onOpenProjectContextMenu.mockClear();
    fireEvent.keyDown(menuBtns[1], { key: 'F10', shiftKey: true });
    expect(onOpenProjectContextMenu).toHaveBeenCalledWith(
      'proj-1',
      expect.objectContaining({ invocation: 'keyboard' })
    );
  });

  it('exposes Inbox hover menu button and moves active project check badge based on activeProjectId', () => {
    const onOpenProjectContextMenu = vi.fn();
    const { container, rerender } = render(
      <ProjectsRail
        {...defaultProps}
        activeProjectId={null}
        onOpenProjectContextMenu={onOpenProjectContextMenu}
      />
    );

    const menuBtns = screen.getAllByRole('button', { name: 'More options' });
    expect(menuBtns).toHaveLength(2);
    fireEvent.click(menuBtns[0], { detail: 1, clientX: 40, clientY: 80 });
    expect(onOpenProjectContextMenu).toHaveBeenCalledWith(
      'untagged',
      expect.objectContaining({ invocation: 'pointer', point: { x: 40, y: 80 } })
    );

    const inboxRow = screen.getByRole('button', { name: /Inbox/i });
    expect(inboxRow.querySelector('.projects-rail-active-badge')).not.toBeNull();
    const projectRow = screen.getByRole('button', { name: /Test Project/i });
    expect(projectRow.querySelector('.projects-rail-active-badge')).toBeNull();

    rerender(
      <ProjectsRail
        {...defaultProps}
        activeProjectId="proj-1"
        onOpenProjectContextMenu={onOpenProjectContextMenu}
      />
    );

    expect(container.querySelectorAll('.projects-rail-active-badge')).toHaveLength(1);
    expect(
      screen.getByRole('button', { name: /Inbox/i }).querySelector('.projects-rail-active-badge')
    ).toBeNull();
    expect(
      screen
        .getByRole('button', { name: /Test Project/i })
        .querySelector('.projects-rail-active-badge')
    ).not.toBeNull();
  });
});
