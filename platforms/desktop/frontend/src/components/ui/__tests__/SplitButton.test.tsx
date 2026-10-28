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

  it('renders two-line card item with title, description, and icon badge when description is provided', () => {
    render(
      <SplitButton
        primaryLabel="Import"
        onPrimaryClick={vi.fn()}
        menuItems={[
          {
            key: 'item-1',
            label: 'Two-line Title',
            description: 'Subtext description',
            icon: <span data-testid="test-icon">icon</span>,
            onClick: vi.fn(),
          },
        ]}
      />
    );

    fireEvent.click(screen.getByRole('button', { name: 'More options' }));

    const menu = screen.getByRole('menu');
    expect(menu.classList.contains('has-descriptions')).toBe(true);

    const item = screen.getByRole('menuitem');
    expect(item.classList.contains('has-description')).toBe(true);

    const titleEl = item.querySelector('.split-button-item-title');
    expect(titleEl?.textContent).toBe('Two-line Title');

    const descEl = item.querySelector('.split-button-item-desc');
    expect(descEl?.textContent).toBe('Subtext description');

    const iconBox = item.querySelector('.split-button-item-icon-box');
    expect(iconBox).toBeDefined();
    expect(iconBox?.contains(screen.getByTestId('test-icon'))).toBe(true);
  });

  it('applies is-secondary class when variant="secondary"', () => {
    const { container: primaryContainer } = render(
      <SplitButton
        primaryLabel="Primary"
        onPrimaryClick={vi.fn()}
        menuItems={[{ key: '1', label: 'Item 1', onClick: vi.fn() }]}
      />
    );
    const primaryRoot = primaryContainer.querySelector('.split-button-container');
    expect(primaryRoot?.classList.contains('is-secondary')).toBe(false);

    const { container: secondaryContainer } = render(
      <SplitButton
        primaryLabel="Secondary"
        variant="secondary"
        onPrimaryClick={vi.fn()}
        menuItems={[{ key: '1', label: 'Item 1', onClick: vi.fn() }]}
      />
    );
    const secondaryRoot = secondaryContainer.querySelector('.split-button-container');
    expect(secondaryRoot?.classList.contains('is-secondary')).toBe(true);
  });

  it('mounts menu in document.body via ModalPortal and computes bounded absolute position', () => {
    const { container } = render(
      <SplitButton
        primaryLabel="Import"
        onPrimaryClick={vi.fn()}
        menuItems={[{ key: 'item-1', label: 'Item 1', onClick: vi.fn() }]}
      />
    );

    const toggleBtn = screen.getByRole('button', { name: 'More options' });
    vi.spyOn(toggleBtn, 'getBoundingClientRect').mockReturnValue({
      top: 100,
      bottom: 130,
      left: 200,
      right: 230,
      width: 30,
      height: 30,
      x: 200,
      y: 100,
      toJSON: () => {},
    });

    fireEvent.click(toggleBtn);

    const menu = screen.getByRole('menu');
    expect(menu).toBeDefined();
    expect(menu.parentElement).toBe(document.body);
    expect(menu.classList.contains('split-button-portal-menu')).toBe(true);
    expect(container.contains(menu)).toBe(false);
    expect(menu.style.top).toBe('134px');
  });

  it('closes dropdown menu when primary button is clicked', () => {
    const onPrimaryClick = vi.fn();
    render(
      <SplitButton
        primaryLabel="Primary"
        onPrimaryClick={onPrimaryClick}
        menuItems={[{ key: '1', label: 'Item 1', onClick: vi.fn() }]}
      />
    );

    const toggleBtn = screen.getByRole('button', { name: 'More options' });
    fireEvent.click(toggleBtn);
    expect(screen.queryByRole('menu')).not.toBeNull();

    const primaryBtn = screen.getByRole('button', { name: 'Primary' });
    fireEvent.click(primaryBtn);

    expect(onPrimaryClick).toHaveBeenCalled();
    expect(screen.queryByRole('menu')).toBeNull();
  });

  it('does not propagate outer className to portaled menu', () => {
    render(
      <SplitButton
        className="custom-outer-class"
        primaryLabel="Primary"
        onPrimaryClick={vi.fn()}
        menuItems={[{ key: '1', label: 'Item 1', onClick: vi.fn() }]}
      />
    );

    const toggleBtn = screen.getByRole('button', { name: 'More options' });
    fireEvent.click(toggleBtn);

    const menu = screen.getByRole('menu');
    expect(menu.classList.contains('custom-outer-class')).toBe(false);
  });

  it('closes menu and restores focus to trigger on Tab key', () => {
    render(
      <SplitButton
        primaryLabel="Primary"
        onPrimaryClick={vi.fn()}
        menuItems={[{ key: '1', label: 'Item 1', onClick: vi.fn() }]}
      />
    );

    const toggleBtn = screen.getByRole('button', { name: 'More options' });
    fireEvent.click(toggleBtn);
    expect(screen.queryByRole('menu')).not.toBeNull();

    fireEvent.keyDown(document, { key: 'Tab' });
    expect(screen.queryByRole('menu')).toBeNull();
  });
});
