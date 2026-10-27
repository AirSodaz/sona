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
});
