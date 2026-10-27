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
});
