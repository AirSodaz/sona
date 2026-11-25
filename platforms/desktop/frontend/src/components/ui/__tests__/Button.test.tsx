import { fireEvent, render, screen } from '@testing-library/react';
import { createRef } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { Button, IconButton } from '../Button';

describe('Button', () => {
  it('renders defaults with type="button", tier="secondary", and size="md"', () => {
    render(<Button>Save</Button>);
    const btn = screen.getByRole('button', { name: 'Save' }) as HTMLButtonElement;
    expect(btn.type).toBe('button');
    expect(btn.className).toContain('btn');
    expect(btn.className).toContain('btn-secondary');
    expect(btn.className).toContain('btn-md');
    expect(btn.className).not.toContain('btn-danger');
    fireEvent.focus(btn);
    expect(screen.queryByRole('tooltip')).toBeNull();
  });

  it('applies orthogonal tier, tone, size, pill, fullWidth, and active classes', () => {
    render(
      <Button tier="primary" tone="danger" size="sm" pill fullWidth active type="submit">
        Delete
      </Button>
    );
    const btn = screen.getByRole('button', { name: 'Delete' }) as HTMLButtonElement;
    expect(btn.type).toBe('submit');
    expect(btn.className).toContain('btn-primary');
    expect(btn.className).toContain('btn-danger');
    expect(btn.className).toContain('btn-sm');
    expect(btn.className).toContain('btn-pill');
    expect(btn.className).toContain('btn-full');
    expect(btn.className).toContain('is-active');
  });

  it('renders decorative leftIcon and rightIcon with aria-hidden', () => {
    render(
      <Button
        leftIcon={<span data-testid="left-ico" />}
        rightIcon={<span data-testid="right-ico" />}
      >
        Action
      </Button>
    );
    const leftWrap = screen.getByTestId('left-ico').parentElement;
    const rightWrap = screen.getByTestId('right-ico').parentElement;
    expect(leftWrap?.getAttribute('aria-hidden')).toBe('true');
    expect(rightWrap?.getAttribute('aria-hidden')).toBe('true');
  });

  it('supports width-preserving loading state, sets aria-busy, and blocks clicks', () => {
    const onClick = vi.fn();
    render(
      <Button tier="primary" loading onClick={onClick}>
        Submitting
      </Button>
    );
    const btn = screen.getByRole('button', { name: 'Submitting' }) as HTMLButtonElement;
    expect(btn.disabled).toBe(true);
    expect(btn.getAttribute('aria-busy')).toBe('true');
    expect(btn.className).toContain('is-loading');
    expect(btn.querySelector('.btn-spinner')).not.toBeNull();
    fireEvent.click(btn);
    expect(onClick).not.toHaveBeenCalled();
  });

  it('applies transient feedback classes without disabling the button', () => {
    const { rerender } = render(
      <Button tier="secondary" feedback="success">
        Copied
      </Button>
    );
    const btn = screen.getByRole('button', { name: 'Copied' }) as HTMLButtonElement;
    expect(btn.disabled).toBe(false);
    expect(btn.className).toContain('btn-feedback-success');

    rerender(
      <Button tier="secondary" feedback="error">
        Copied
      </Button>
    );
    expect(btn.disabled).toBe(false);
    expect(btn.className).toContain('btn-feedback-error');
  });

  it('renders Portal Tooltip on disabled Button when tooltip prop is provided', () => {
    render(
      <Button tier="secondary" tone="danger" disabled tooltip="Active project cannot be deleted">
        Delete Project
      </Button>
    );
    const btn = screen.getByRole('button', { name: 'Delete Project' }) as HTMLButtonElement;
    expect(btn.disabled).toBe(true);
    const wrapper = btn.parentElement as HTMLElement;
    fireEvent.focus(wrapper);
    expect(screen.getByRole('tooltip').textContent).toBe('Active project cannot be deleted');
  });

  it('forwards ref to the underlying button element', () => {
    const ref = createRef<HTMLButtonElement>();
    render(<Button ref={ref}>RefTarget</Button>);
    expect(ref.current).toBeInstanceOf(HTMLButtonElement);
    expect(ref.current?.textContent).toBe('RefTarget');
  });
});

describe('IconButton', () => {
  it('defaults to ghost tier, sm size, square btn-icon class, and auto Portal Tooltip from aria-label', () => {
    render(<IconButton aria-label="Close modal" icon={<span data-testid="x-icon" />} />);
    const btn = screen.getByRole('button', { name: 'Close modal' }) as HTMLButtonElement;
    expect(btn.type).toBe('button');
    expect(btn.className).toContain('btn');
    expect(btn.className).toContain('btn-ghost');
    expect(btn.className).toContain('btn-sm');
    expect(btn.className).toContain('btn-icon');
    expect(screen.getByTestId('x-icon').parentElement?.getAttribute('aria-hidden')).toBe('true');

    fireEvent.focus(btn);
    expect(screen.getByRole('tooltip').textContent).toBe('Close modal');
  });

  it('supports custom tooltip override and tooltip={false} opt-out', () => {
    const { rerender } = render(
      <IconButton aria-label="Undo" tooltip="Undo (Ctrl+Z)" icon={<span>U</span>} />
    );
    const btn = screen.getByRole('button', { name: 'Undo' });
    fireEvent.focus(btn);
    expect(screen.getByRole('tooltip').textContent).toBe('Undo (Ctrl+Z)');

    rerender(<IconButton aria-label="Undo" tooltip={false} icon={<span>U</span>} />);
    fireEvent.blur(btn);
    fireEvent.focus(btn);
    expect(screen.queryByRole('tooltip')).toBeNull();
  });

  it('supports ghost danger and xs size', () => {
    render(
      <IconButton
        tier="ghost"
        tone="danger"
        size="xs"
        aria-label="Delete item"
        icon={<span>trash</span>}
      />
    );
    const btn = screen.getByRole('button', { name: 'Delete item' });
    expect(btn.className).toContain('btn-ghost');
    expect(btn.className).toContain('btn-danger');
    expect(btn.className).toContain('btn-xs');
    expect(btn.className).toContain('btn-icon');
  });
});
