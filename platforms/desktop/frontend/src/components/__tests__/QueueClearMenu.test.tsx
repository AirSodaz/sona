import { fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { createReactI18nextMock } from '../../__tests__/testUtils/i18n';
import { QueueClearMenu } from '../QueueClearMenu';

vi.mock('react-i18next', () => createReactI18nextMock());

describe('QueueClearMenu', () => {
  const onClear = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders trigger button and toggles menu on click', () => {
    render(<QueueClearMenu completedCount={2} totalCount={5} onClear={onClear} />);

    const trigger = screen.getByTestId('queue-clear-trigger');
    expect(trigger.getAttribute('aria-expanded')).toBe('false');
    expect(screen.queryByRole('menu')).toBeNull();

    fireEvent.click(trigger);
    expect(trigger.getAttribute('aria-expanded')).toBe('true');
    expect(screen.getByRole('menu')).toBeDefined();

    fireEvent.click(trigger);
    expect(trigger.getAttribute('aria-expanded')).toBe('false');
    expect(screen.queryByRole('menu')).toBeNull();
  });

  it('calls onClear with "completed" when clicking Clear Completed', () => {
    render(<QueueClearMenu completedCount={3} totalCount={5} onClear={onClear} />);

    fireEvent.click(screen.getByTestId('queue-clear-trigger'));
    const clearCompletedBtn = screen.getByRole('menuitem', { name: /batch\.clear_completed/ });
    expect(clearCompletedBtn.hasAttribute('disabled')).toBe(false);

    fireEvent.click(clearCompletedBtn);
    expect(onClear).toHaveBeenCalledWith('completed');
    expect(screen.queryByRole('menu')).toBeNull();
  });

  it('disables Clear Completed when completedCount is 0', () => {
    render(<QueueClearMenu completedCount={0} totalCount={3} onClear={onClear} />);

    fireEvent.click(screen.getByTestId('queue-clear-trigger'));
    const clearCompletedBtn = screen.getByRole('menuitem', { name: /batch\.clear_completed/ });
    expect(clearCompletedBtn.hasAttribute('disabled')).toBe(true);

    fireEvent.click(clearCompletedBtn);
    expect(onClear).not.toHaveBeenCalled();
  });

  it('calls onClear with "all" when clicking Clear All', () => {
    render(<QueueClearMenu completedCount={1} totalCount={4} onClear={onClear} />);

    fireEvent.click(screen.getByTestId('queue-clear-trigger'));
    const clearAllBtn = screen.getByRole('menuitem', { name: /batch\.clear_all/ });
    fireEvent.click(clearAllBtn);
    expect(onClear).toHaveBeenCalledWith('all');
    expect(screen.queryByRole('menu')).toBeNull();
  });

  it('closes menu and refocuses trigger on Escape key', () => {
    render(<QueueClearMenu completedCount={2} totalCount={5} onClear={onClear} />);

    const trigger = screen.getByTestId('queue-clear-trigger');
    fireEvent.click(trigger);
    expect(screen.getByRole('menu')).toBeDefined();

    fireEvent.keyDown(document, { key: 'Escape' });
    expect(screen.queryByRole('menu')).toBeNull();
  });

  it('closes menu when clicking outside', () => {
    render(
      <div>
        <div data-testid="outside-area">Outside</div>
        <QueueClearMenu completedCount={2} totalCount={5} onClear={onClear} />
      </div>
    );

    fireEvent.click(screen.getByTestId('queue-clear-trigger'));
    expect(screen.getByRole('menu')).toBeDefined();

    fireEvent.mouseDown(screen.getByTestId('outside-area'));
    expect(screen.queryByRole('menu')).toBeNull();
  });
});
