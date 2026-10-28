import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { SplitButton } from '../SplitButton';

describe('SplitButton', () => {
  it('renders primary button and toggle button with parameterized aria-label', () => {
    const handlePrimary = vi.fn();
    render(
      <SplitButton
        primaryLabel="Import"
        ariaLabel="Import files"
        onPrimaryClick={handlePrimary}
        menuItems={[{ key: 'item-1', label: 'Item 1', onClick: vi.fn() }]}
      />
    );

    const primaryBtn = screen.getByRole('button', { name: 'Import files' });
    expect(primaryBtn).toBeDefined();

    const toggleBtn = screen.getByRole('button', { name: 'Import files options' });
    expect(toggleBtn).toBeDefined();

    fireEvent.click(primaryBtn);
    expect(handlePrimary).toHaveBeenCalledTimes(1);
  });

  it('closes dropdown and focuses trigger on Escape key with event stopped', () => {
    render(
      <SplitButton
        primaryLabel="Import"
        onPrimaryClick={vi.fn()}
        menuItems={[{ key: 'item-1', label: 'Item 1', onClick: vi.fn() }]}
      />
    );

    const toggleBtn = screen.getByRole('button', { name: 'More options' });
    fireEvent.click(toggleBtn);

    expect(screen.getByRole('menu')).toBeDefined();

    fireEvent.keyDown(document, { key: 'Escape' });

    expect(screen.queryByRole('menu')).toBeNull();
    expect(document.activeElement).toBe(toggleBtn);
  });

  it('calls menuItem onClick and closes menu when item clicked', () => {
    const handleItem1 = vi.fn();
    render(
      <SplitButton
        primaryLabel="Import"
        onPrimaryClick={vi.fn()}
        menuItems={[{ key: 'item-1', label: 'Item 1', onClick: handleItem1 }]}
      />
    );

    fireEvent.click(screen.getByRole('button', { name: 'More options' }));
    const menuItem = screen.getByRole('menuitem', { name: 'Item 1' });
    fireEvent.click(menuItem);

    expect(handleItem1).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole('menu')).toBeNull();
  });

  it('navigates menu items with ArrowDown and ArrowUp keys', () => {
    render(
      <SplitButton
        primaryLabel="Import"
        onPrimaryClick={vi.fn()}
        menuItems={[
          { key: 'item-1', label: 'Item 1', onClick: vi.fn() },
          { key: 'item-2', label: 'Item 2', onClick: vi.fn() },
        ]}
      />
    );

    const toggleBtn = screen.getByRole('button', { name: 'More options' });
    fireEvent.click(toggleBtn);

    const item1 = screen.getByRole('menuitem', { name: 'Item 1' });
    const item2 = screen.getByRole('menuitem', { name: 'Item 2' });

    expect(document.activeElement).toBe(item1);

    fireEvent.keyDown(document, { key: 'ArrowDown' });
    expect(document.activeElement).toBe(item2);

    fireEvent.keyDown(document, { key: 'ArrowDown' });
    expect(document.activeElement).toBe(item1);

    fireEvent.keyDown(document, { key: 'ArrowUp' });
    expect(document.activeElement).toBe(item2);
  });

  it('closes dropdown when clicking outside', () => {
    render(
      <div>
        <div data-testid="outside">Outside</div>
        <SplitButton
          primaryLabel="Import"
          onPrimaryClick={vi.fn()}
          menuItems={[{ key: 'item-1', label: 'Item 1', onClick: vi.fn() }]}
        />
      </div>
    );

    fireEvent.click(screen.getByRole('button', { name: 'More options' }));
    expect(screen.getByRole('menu')).toBeDefined();

    fireEvent.mouseDown(screen.getByTestId('outside'));
    expect(screen.queryByRole('menu')).toBeNull();
  });

  it('disables trigger when menuItems is empty or component disabled', () => {
    const { rerender } = render(
      <SplitButton primaryLabel="Import" onPrimaryClick={vi.fn()} menuItems={[]} />
    );
    const toggleBtn = screen.getByRole('button', { name: 'More options' }) as HTMLButtonElement;
    expect(toggleBtn.disabled).toBe(true);

    rerender(
      <SplitButton
        primaryLabel="Import"
        disabled
        onPrimaryClick={vi.fn()}
        menuItems={[{ key: 'item-1', label: 'Item 1', onClick: vi.fn() }]}
      />
    );
    expect((screen.getByRole('button', { name: 'Import' }) as HTMLButtonElement).disabled).toBe(
      true
    );
    expect(
      (screen.getByRole('button', { name: 'More options' }) as HTMLButtonElement).disabled
    ).toBe(true);
  });

  it('opens menu with ArrowDown key on trigger and navigates with Home/End', () => {
    render(
      <SplitButton
        primaryLabel="Import"
        onPrimaryClick={vi.fn()}
        menuItems={[
          { key: 'item-1', label: 'Item 1', onClick: vi.fn() },
          { key: 'item-2', label: 'Item 2', onClick: vi.fn() },
          { key: 'item-3', label: 'Item 3', onClick: vi.fn() },
        ]}
      />
    );

    const toggleBtn = screen.getByRole('button', { name: 'More options' });
    expect(toggleBtn.getAttribute('aria-expanded')).toBe('false');
    expect(toggleBtn.getAttribute('aria-controls')).toBeNull();

    fireEvent.keyDown(toggleBtn, { key: 'ArrowDown' });
    expect(toggleBtn.getAttribute('aria-expanded')).toBe('true');
    expect(toggleBtn.getAttribute('aria-controls')).toBeTruthy();

    const item1 = screen.getByRole('menuitem', { name: 'Item 1' });
    const item3 = screen.getByRole('menuitem', { name: 'Item 3' });

    fireEvent.keyDown(document, { key: 'End' });
    expect(document.activeElement).toBe(item3);

    fireEvent.keyDown(document, { key: 'Home' });
    expect(document.activeElement).toBe(item1);
  });

  it('skips disabled items during arrow navigation', () => {
    render(
      <SplitButton
        primaryLabel="Import"
        onPrimaryClick={vi.fn()}
        menuItems={[
          { key: 'item-1', label: 'Item 1', onClick: vi.fn() },
          { key: 'item-2', label: 'Item 2', disabled: true, onClick: vi.fn() },
          { key: 'item-3', label: 'Item 3', onClick: vi.fn() },
        ]}
      />
    );

    fireEvent.click(screen.getByRole('button', { name: 'More options' }));
    const item1 = screen.getByRole('menuitem', { name: 'Item 1' });
    const item3 = screen.getByRole('menuitem', { name: 'Item 3' });

    expect(document.activeElement).toBe(item1);
    fireEvent.keyDown(document, { key: 'ArrowDown' });
    expect(document.activeElement).toBe(item3);
  });
});
