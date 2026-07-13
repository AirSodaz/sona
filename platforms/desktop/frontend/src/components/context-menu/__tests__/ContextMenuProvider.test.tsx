import { readFileSync } from 'node:fs';
import React from 'react';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import {
  ContextMenuProvider,
} from '../ContextMenuProvider';
import { useContextMenu } from '../useContextMenu';

const contextMenuCss = readFileSync('src/styles/context-menu.css', 'utf8');

function ContextMenuHarness(): React.JSX.Element {
  const { activeContextId, openContextMenu } = useContextMenu();

  return (
    <div>
      <button
        type="button"
        onClick={(event) => {
          openContextMenu({
            contextId: 'test:item:alpha',
            ariaLabel: 'Actions for Alpha',
            actions: [
              {
                id: 'open',
                label: 'Open',
                onSelect: vi.fn(),
              },
            ],
            anchor: event.currentTarget,
            point: { x: 24, y: 32 },
            invocation: 'pointer',
          });
        }}
      >
        Open menu
      </button>
      <output aria-label="Active context">{activeContextId ?? 'none'}</output>
    </div>
  );
}

describe('ContextMenuProvider', () => {
  it('renders the active menu through a portal and exposes its context id', () => {
    render(
      <ContextMenuProvider>
        <ContextMenuHarness />
      </ContextMenuProvider>,
    );

    fireEvent.click(screen.getByRole('button', { name: 'Open menu' }));

    expect(screen.getByRole('menu', { name: 'Actions for Alpha' })).toBeDefined();
    expect(screen.getByRole('menuitem', { name: 'Open' })).toBeDefined();
    expect(screen.getByLabelText('Active context').textContent).toBe('test:item:alpha');
    expect(document.body.querySelector('.context-menu')).not.toBeNull();
  });

  it('replaces the previous menu and reports the close reason', () => {
    const onFirstClose = vi.fn();

    function ReplacementHarness(): React.JSX.Element {
      const { openContextMenu } = useContextMenu();

      const open = (
        event: React.MouseEvent<HTMLButtonElement>,
        contextId: string,
        label: string,
      ) => {
        openContextMenu({
          contextId,
          ariaLabel: label,
          actions: [{ id: 'open', label: 'Open', onSelect: vi.fn() }],
          anchor: event.currentTarget,
          point: { x: 12, y: 18 },
          invocation: 'pointer',
          onClose: contextId === 'first' ? onFirstClose : undefined,
        });
      };

      return (
        <>
          <button type="button" onClick={(event) => open(event, 'first', 'First menu')}>First</button>
          <button type="button" onClick={(event) => open(event, 'second', 'Second menu')}>Second</button>
        </>
      );
    }

    render(
      <ContextMenuProvider>
        <ReplacementHarness />
      </ContextMenuProvider>,
    );

    fireEvent.click(screen.getByRole('button', { name: 'First' }));
    fireEvent.click(screen.getByRole('button', { name: 'Second' }));

    expect(onFirstClose).toHaveBeenCalledWith('replaced');
    expect(screen.queryByRole('menu', { name: 'First menu' })).toBeNull();
    expect(screen.getAllByRole('menu')).toHaveLength(1);
    expect(screen.getByRole('menu', { name: 'Second menu' })).toBeDefined();
  });

  it('allows a replaced menu close callback to close re-entrantly without closing it twice', () => {
    const closeReasons: string[] = [];

    function ReentrantReplacementHarness(): React.JSX.Element {
      const { closeContextMenu, openContextMenu } = useContextMenu();

      const open = (
        event: React.MouseEvent<HTMLButtonElement>,
        contextId: string,
        label: string,
      ) => {
        openContextMenu({
          contextId,
          ariaLabel: label,
          actions: [{ id: 'open', label: 'Open', onSelect: vi.fn() }],
          anchor: event.currentTarget,
          point: { x: 12, y: 18 },
          invocation: 'pointer',
          onClose: contextId === 'first'
            ? (reason) => {
                closeReasons.push(reason);
                closeContextMenu();
              }
            : undefined,
        });
      };

      return (
        <>
          <button type="button" onClick={(event) => open(event, 'first', 'First menu')}>First</button>
          <button type="button" onClick={(event) => open(event, 'second', 'Second menu')}>Second</button>
        </>
      );
    }

    render(
      <ContextMenuProvider>
        <ReentrantReplacementHarness />
      </ContextMenuProvider>,
    );

    fireEvent.click(screen.getByRole('button', { name: 'First' }));
    fireEvent.click(screen.getByRole('button', { name: 'Second' }));

    expect(closeReasons).toEqual(['replaced']);
    expect(screen.getAllByRole('menu')).toHaveLength(1);
    expect(screen.getByRole('menu', { name: 'Second menu' })).toBeDefined();
  });

  it('closes with the action reason before invoking the selected action', () => {
    const calls: string[] = [];

    function ActionHarness(): React.JSX.Element {
      const { activeContextId, openContextMenu } = useContextMenu();

      return (
        <>
          <button
            type="button"
            onClick={(event) => {
              openContextMenu({
                contextId: 'action-menu',
                ariaLabel: 'Action menu',
                actions: [{
                  id: 'rename',
                  label: 'Rename',
                  onSelect: () => calls.push('select'),
                }],
                anchor: event.currentTarget,
                point: { x: 20, y: 24 },
                invocation: 'pointer',
                onClose: (reason) => calls.push(`close:${reason}`),
              });
            }}
          >
            Show actions
          </button>
          <output aria-label="Action context">{activeContextId ?? 'none'}</output>
        </>
      );
    }

    render(
      <ContextMenuProvider>
        <ActionHarness />
      </ContextMenuProvider>,
    );

    fireEvent.click(screen.getByRole('button', { name: 'Show actions' }));
    fireEvent.click(screen.getByRole('menuitem', { name: 'Rename' }));

    expect(calls).toEqual(['close:action', 'select']);
    expect(screen.queryByRole('menu')).toBeNull();
    expect(screen.getByLabelText('Action context').textContent).toBe('none');
  });

  it('closes programmatically through the controller and clears the active context id', () => {
    const onClose = vi.fn();

    function ProgrammaticCloseHarness(): React.JSX.Element {
      const { activeContextId, closeContextMenu, openContextMenu } = useContextMenu();

      return (
        <>
          <button
            type="button"
            onClick={(event) => openContextMenu({
              contextId: 'programmatic-menu',
              ariaLabel: 'Programmatic menu',
              actions: [{ id: 'open', label: 'Open', onSelect: vi.fn() }],
              anchor: event.currentTarget,
              point: { x: 12, y: 12 },
              invocation: 'pointer',
              onClose,
            })}
          >
            Open programmatic menu
          </button>
          <button type="button" onClick={closeContextMenu}>Close programmatic menu</button>
          <output aria-label="Programmatic context">{activeContextId ?? 'none'}</output>
        </>
      );
    }

    render(
      <ContextMenuProvider>
        <ProgrammaticCloseHarness />
      </ContextMenuProvider>,
    );

    fireEvent.click(screen.getByRole('button', { name: 'Open programmatic menu' }));
    expect(screen.getByLabelText('Programmatic context').textContent).toBe('programmatic-menu');

    fireEvent.click(screen.getByRole('button', { name: 'Close programmatic menu' }));

    expect(onClose).toHaveBeenCalledOnce();
    expect(onClose).toHaveBeenCalledWith('programmatic');
    expect(screen.queryByRole('menu')).toBeNull();
    expect(screen.getByLabelText('Programmatic context').textContent).toBe('none');
  });

  it('focuses the first enabled action and navigates enabled actions with the keyboard', async () => {
    function KeyboardHarness(): React.JSX.Element {
      const { openContextMenu } = useContextMenu();

      return (
        <button
          type="button"
          onClick={(event) => {
            openContextMenu({
              contextId: 'keyboard-menu',
              ariaLabel: 'Keyboard menu',
              actions: [
                { id: 'disabled', label: 'Disabled', disabled: true, onSelect: vi.fn() },
                { id: 'open', label: 'Open', onSelect: vi.fn() },
                { id: 'rename', label: 'Rename', onSelect: vi.fn() },
              ],
              anchor: event.currentTarget,
              point: { x: 10, y: 10 },
              invocation: 'keyboard',
            });
          }}
        >
          Keyboard trigger
        </button>
      );
    }

    render(
      <ContextMenuProvider>
        <KeyboardHarness />
      </ContextMenuProvider>,
    );

    fireEvent.click(screen.getByRole('button', { name: 'Keyboard trigger' }));
    const openAction = screen.getByRole('menuitem', { name: 'Open' });
    const renameAction = screen.getByRole('menuitem', { name: 'Rename' });

    await waitFor(() => expect(document.activeElement).toBe(openAction));

    fireEvent.keyDown(openAction, { key: 'ArrowDown' });
    expect(document.activeElement).toBe(renameAction);
    fireEvent.keyDown(renameAction, { key: 'ArrowDown' });
    expect(document.activeElement).toBe(openAction);
    fireEvent.keyDown(openAction, { key: 'ArrowUp' });
    expect(document.activeElement).toBe(renameAction);
    fireEvent.keyDown(renameAction, { key: 'Home' });
    expect(document.activeElement).toBe(openAction);
    fireEvent.keyDown(openAction, { key: 'End' });
    expect(document.activeElement).toBe(renameAction);
  });

  it('focuses an all-disabled menu so Escape can close it and restore the anchor', async () => {
    const onClose = vi.fn();

    function DisabledHarness(): React.JSX.Element {
      const { openContextMenu } = useContextMenu();

      return (
        <button
          type="button"
          onClick={(event) => {
            openContextMenu({
              contextId: 'disabled-menu',
              ariaLabel: 'Disabled menu',
              actions: [
                { id: 'open', label: 'Open', disabled: true, onSelect: vi.fn() },
                { id: 'settings', label: 'Settings', disabled: true, onSelect: vi.fn() },
              ],
              anchor: event.currentTarget,
              point: { x: 12, y: 12 },
              invocation: 'keyboard',
              onClose,
            });
          }}
        >
          Disabled trigger
        </button>
      );
    }

    render(
      <ContextMenuProvider>
        <DisabledHarness />
      </ContextMenuProvider>,
    );

    const trigger = screen.getByRole('button', { name: 'Disabled trigger' });
    trigger.focus();
    fireEvent.click(trigger);
    const menu = screen.getByRole('menu', { name: 'Disabled menu' });

    await waitFor(() => expect(document.activeElement).toBe(menu));
    fireEvent.keyDown(menu, { key: 'Escape' });

    expect(onClose).toHaveBeenCalledWith('escape');
    expect(screen.queryByRole('menu')).toBeNull();
    expect(document.activeElement).toBe(trigger);
  });

  it('closes on Escape and restores focus to the connected anchor', async () => {
    const onClose = vi.fn();

    function EscapeHarness(): React.JSX.Element {
      const { openContextMenu } = useContextMenu();

      return (
        <button
          type="button"
          onClick={(event) => {
            openContextMenu({
              contextId: 'escape-menu',
              ariaLabel: 'Escape menu',
              actions: [{ id: 'open', label: 'Open', onSelect: vi.fn() }],
              anchor: event.currentTarget,
              point: { x: 16, y: 20 },
              invocation: 'keyboard',
              onClose,
            });
          }}
        >
          Escape trigger
        </button>
      );
    }

    render(
      <ContextMenuProvider>
        <EscapeHarness />
      </ContextMenuProvider>,
    );

    const trigger = screen.getByRole('button', { name: 'Escape trigger' });
    trigger.focus();
    fireEvent.click(trigger);
    const action = screen.getByRole('menuitem', { name: 'Open' });
    await waitFor(() => expect(document.activeElement).toBe(action));

    fireEvent.keyDown(action, { key: 'Escape' });

    expect(onClose).toHaveBeenCalledWith('escape');
    expect(screen.queryByRole('menu')).toBeNull();
    expect(document.activeElement).toBe(trigger);
  });

  it('does not focus an anchor removed from the DOM when Escape closes the menu', async () => {
    let anchorFocus: ReturnType<typeof vi.spyOn> | null = null;

    function RemovedAnchorHarness(): React.JSX.Element {
      const [showTrigger, setShowTrigger] = React.useState(true);
      const { openContextMenu } = useContextMenu();

      return showTrigger ? (
        <button
          type="button"
          onClick={(event) => {
            anchorFocus = vi.spyOn(event.currentTarget, 'focus');
            openContextMenu({
              contextId: 'removed-anchor-menu',
              ariaLabel: 'Removed anchor menu',
              actions: [{ id: 'open', label: 'Open', onSelect: vi.fn() }],
              anchor: event.currentTarget,
              point: { x: 16, y: 20 },
              invocation: 'keyboard',
            });
            setShowTrigger(false);
          }}
        >
          Removed anchor trigger
        </button>
      ) : <span>Trigger removed</span>;
    }

    render(
      <ContextMenuProvider>
        <RemovedAnchorHarness />
      </ContextMenuProvider>,
    );

    fireEvent.click(screen.getByRole('button', { name: 'Removed anchor trigger' }));
    const action = screen.getByRole('menuitem', { name: 'Open' });
    await waitFor(() => expect(document.activeElement).toBe(action));

    fireEvent.keyDown(action, { key: 'Escape' });

    expect(screen.queryByRole('menu')).toBeNull();
    expect(anchorFocus).not.toBeNull();
    expect(anchorFocus).not.toHaveBeenCalled();
  });

  it('reports global dismissal reasons and ignores pointer events inside the menu', () => {
    const onClose = vi.fn();

    function GlobalDismissHarness(): React.JSX.Element {
      const { openContextMenu } = useContextMenu();

      return (
        <button
          type="button"
          onClick={(event) => {
            openContextMenu({
              contextId: 'global-dismiss-menu',
              ariaLabel: 'Global dismiss menu',
              actions: [{ id: 'open', label: 'Open', onSelect: vi.fn() }],
              anchor: event.currentTarget,
              point: { x: 8, y: 12 },
              invocation: 'pointer',
              onClose,
            });
          }}
        >
          Global trigger
        </button>
      );
    }

    render(
      <ContextMenuProvider>
        <GlobalDismissHarness />
      </ContextMenuProvider>,
    );

    const trigger = screen.getByRole('button', { name: 'Global trigger' });
    const openMenu = () => fireEvent.click(trigger);

    openMenu();
    fireEvent.pointerDown(screen.getByRole('menuitem', { name: 'Open' }));
    expect(screen.getByRole('menu')).toBeDefined();

    fireEvent.pointerDown(document.body);
    expect(onClose).toHaveBeenLastCalledWith('outside');
    expect(screen.queryByRole('menu')).toBeNull();

    openMenu();
    fireEvent.contextMenu(document.body);
    expect(onClose).toHaveBeenLastCalledWith('outside');

    openMenu();
    fireEvent.scroll(window);
    expect(onClose).toHaveBeenLastCalledWith('scroll');

    openMenu();
    fireEvent(window, new Event('resize'));
    expect(onClose).toHaveBeenLastCalledWith('resize');

    openMenu();
    fireEvent.blur(window);
    expect(onClose).toHaveBeenLastCalledWith('blur');
    expect(onClose).toHaveBeenCalledTimes(5);
  });

  it('closes when a nested scrolling element scrolls', () => {
    const onClose = vi.fn();

    function NestedScrollHarness(): React.JSX.Element {
      const { openContextMenu } = useContextMenu();

      return (
        <div data-testid="nested-scroll-container">
          <button
            type="button"
            onClick={(event) => openContextMenu({
              contextId: 'nested-scroll-menu',
              ariaLabel: 'Nested scroll menu',
              actions: [{ id: 'open', label: 'Open', onSelect: vi.fn() }],
              anchor: event.currentTarget,
              point: { x: 8, y: 12 },
              invocation: 'pointer',
              onClose,
            })}
          >
            Nested scroll trigger
          </button>
        </div>
      );
    }

    render(
      <ContextMenuProvider>
        <NestedScrollHarness />
      </ContextMenuProvider>,
    );

    fireEvent.click(screen.getByRole('button', { name: 'Nested scroll trigger' }));
    fireEvent.scroll(screen.getByTestId('nested-scroll-container'));

    expect(onClose).toHaveBeenCalledWith('scroll');
    expect(screen.queryByRole('menu')).toBeNull();
  });

  it('clamps the menu position inside the viewport with an eight pixel margin', () => {
    const rectSpy = vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect')
      .mockImplementation(function getRect(this: HTMLElement) {
        const width = this.classList.contains('context-menu') ? 180 : 0;
        const height = this.classList.contains('context-menu') ? 160 : 0;
        return {
          x: 0,
          y: 0,
          width,
          height,
          top: 0,
          right: width,
          bottom: height,
          left: 0,
          toJSON: () => ({}),
        };
      });
    const previousWidth = window.innerWidth;
    const previousHeight = window.innerHeight;
    Object.defineProperty(window, 'innerWidth', { configurable: true, value: 1000 });
    Object.defineProperty(window, 'innerHeight', { configurable: true, value: 800 });

    function PositionHarness(): React.JSX.Element {
      const { openContextMenu } = useContextMenu();
      return (
        <button
          type="button"
          onClick={(event) => openContextMenu({
            contextId: 'position-menu',
            ariaLabel: 'Position menu',
            actions: [{ id: 'open', label: 'Open', onSelect: vi.fn() }],
            anchor: event.currentTarget,
            point: { x: 950, y: 750 },
            invocation: 'pointer',
          })}
        >
          Position trigger
        </button>
      );
    }

    render(
      <ContextMenuProvider>
        <PositionHarness />
      </ContextMenuProvider>,
    );

    fireEvent.click(screen.getByRole('button', { name: 'Position trigger' }));
    const menu = screen.getByRole('menu', { name: 'Position menu' });

    expect(menu.style.left).toBe('812px');
    expect(menu.style.top).toBe('632px');

    rectSpy.mockRestore();
    Object.defineProperty(window, 'innerWidth', { configurable: true, value: previousWidth });
    Object.defineProperty(window, 'innerHeight', { configurable: true, value: previousHeight });
  });

  it('clamps negative menu coordinates to the top-left eight pixel margin', () => {
    const rectSpy = vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect')
      .mockImplementation(function getRect(this: HTMLElement) {
        const width = this.classList.contains('context-menu') ? 180 : 0;
        const height = this.classList.contains('context-menu') ? 160 : 0;
        return {
          x: 0,
          y: 0,
          width,
          height,
          top: 0,
          right: width,
          bottom: height,
          left: 0,
          toJSON: () => ({}),
        };
      });

    function NegativePositionHarness(): React.JSX.Element {
      const { openContextMenu } = useContextMenu();
      return (
        <button
          type="button"
          onClick={(event) => openContextMenu({
            contextId: 'negative-position-menu',
            ariaLabel: 'Negative position menu',
            actions: [{ id: 'open', label: 'Open', onSelect: vi.fn() }],
            anchor: event.currentTarget,
            point: { x: -50, y: -30 },
            invocation: 'pointer',
          })}
        >
          Negative position trigger
        </button>
      );
    }

    render(
      <ContextMenuProvider>
        <NegativePositionHarness />
      </ContextMenuProvider>,
    );

    fireEvent.click(screen.getByRole('button', { name: 'Negative position trigger' }));
    const menu = screen.getByRole('menu', { name: 'Negative position menu' });

    expect(menu.style.left).toBe('8px');
    expect(menu.style.top).toBe('8px');

    rectSpy.mockRestore();
  });

  it('renders icons, shortcut text, dividers, and danger styling from action descriptors', () => {
    function DescriptorHarness(): React.JSX.Element {
      const { openContextMenu } = useContextMenu();
      return (
        <button
          type="button"
          onClick={(event) => openContextMenu({
            contextId: 'descriptor-menu',
            ariaLabel: 'Descriptor menu',
            actions: [{
              id: 'delete',
              label: 'Delete',
              icon: <span data-testid="delete-icon">icon</span>,
              shortcut: 'Delete',
              dividerBefore: true,
              tone: 'danger',
              onSelect: vi.fn(),
            }],
            anchor: event.currentTarget,
            point: { x: 12, y: 12 },
            invocation: 'pointer',
          })}
        >
          Descriptor trigger
        </button>
      );
    }

    render(
      <ContextMenuProvider>
        <DescriptorHarness />
      </ContextMenuProvider>,
    );

    fireEvent.click(screen.getByRole('button', { name: 'Descriptor trigger' }));
    const action = screen.getByRole('menuitem', { name: 'Delete' });

    expect(screen.getByTestId('delete-icon')).toBeDefined();
    expect(screen.getByText('Delete', { selector: '.context-menu-item-shortcut' })).toBeDefined();
    expect(action.classList.contains('context-menu-item--danger')).toBe(true);
    expect(action.classList.contains('context-menu-item--with-divider')).toBe(true);
  });

  it('keeps the menu above workspace floats and below the lowest modal layer', () => {
    const zIndex = Number(contextMenuCss.match(/\.context-menu\s*\{[^}]*z-index:\s*(\d+)/s)?.[1]);

    expect(zIndex).toBeGreaterThan(120);
    expect(zIndex).toBeLessThan(1000);
  });

  it('reports a programmatic close when the provider unmounts with an open menu', () => {
    const onClose = vi.fn();

    function UnmountHarness(): React.JSX.Element {
      const { openContextMenu } = useContextMenu();
      return (
        <button
          type="button"
          onClick={(event) => openContextMenu({
            contextId: 'unmount-menu',
            ariaLabel: 'Unmount menu',
            actions: [{ id: 'open', label: 'Open', onSelect: vi.fn() }],
            anchor: event.currentTarget,
            point: { x: 12, y: 12 },
            invocation: 'pointer',
            onClose,
          })}
        >
          Unmount trigger
        </button>
      );
    }

    const { unmount } = render(
      <ContextMenuProvider>
        <UnmountHarness />
      </ContextMenuProvider>,
    );

    fireEvent.click(screen.getByRole('button', { name: 'Unmount trigger' }));
    unmount();

    expect(onClose).toHaveBeenCalledWith('programmatic');
  });
});
