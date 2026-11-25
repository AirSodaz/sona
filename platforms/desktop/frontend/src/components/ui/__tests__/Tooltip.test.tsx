import { act, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { Tooltip } from '../Tooltip';

describe('Tooltip', () => {
  afterEach(() => {
    vi.useRealTimers();
    vi.restoreAllMocks();
  });

  it('opens in a body portal after 250ms hover delay and closes on mouseLeave', () => {
    vi.useFakeTimers();

    render(
      <Tooltip content="Save changes" placement="bottom">
        <button type="button">Save</button>
      </Tooltip>
    );

    const trigger = screen.getByRole('button', { name: 'Save' });
    expect(screen.queryByRole('tooltip')).toBeNull();

    fireEvent.mouseEnter(trigger);
    expect(screen.queryByRole('tooltip')).toBeNull();

    act(() => {
      vi.advanceTimersByTime(249);
    });
    expect(screen.queryByRole('tooltip')).toBeNull();

    act(() => {
      vi.advanceTimersByTime(1);
    });
    const tooltip = screen.getByRole('tooltip');
    expect(tooltip.textContent).toBe('Save changes');
    expect(tooltip.parentElement).toBe(document.body);
    expect(trigger.getAttribute('aria-describedby')).toBe(tooltip.id);

    fireEvent.mouseLeave(trigger);
    expect(screen.queryByRole('tooltip')).toBeNull();
    expect(trigger.getAttribute('aria-describedby')).toBeNull();
  });

  it('opens immediately on keyboard focus and closes on Escape or blur', () => {
    render(
      <Tooltip content="Keyboard hint" placement="top" multiline>
        <button type="button">Action</button>
      </Tooltip>
    );

    const trigger = screen.getByRole('button', { name: 'Action' });
    fireEvent.focus(trigger);

    const tooltip = screen.getByRole('tooltip');
    expect(tooltip.textContent).toBe('Keyboard hint');
    expect(tooltip.className).toContain('tooltip-portal--multiline');

    fireEvent.keyDown(window, { key: 'Escape' });
    expect(screen.queryByRole('tooltip')).toBeNull();

    fireEvent.focus(trigger);
    expect(screen.getByRole('tooltip')).toBeDefined();

    fireEvent.blur(trigger);
    expect(screen.queryByRole('tooltip')).toBeNull();
  });

  it('does not open on mouseDown + focus click sequence', () => {
    render(
      <Tooltip content="Clicked button">
        <button type="button">Click me</button>
      </Tooltip>
    );

    const trigger = screen.getByRole('button', { name: 'Click me' });
    fireEvent.mouseDown(trigger);
    fireEvent.focus(trigger);
    expect(screen.queryByRole('tooltip')).toBeNull();
  });

  it('clears pointerDown guard on mouseLeave/mouseUp so subsequent keyboard focus opens tooltip', () => {
    render(
      <Tooltip content="Keyboard hint">
        <button type="button">Action</button>
      </Tooltip>
    );

    const trigger = screen.getByRole('button', { name: 'Action' });
    fireEvent.mouseDown(trigger);
    fireEvent.mouseLeave(trigger);
    fireEvent.mouseUp(trigger);

    fireEvent.focus(trigger);
    expect(screen.getByRole('tooltip').textContent).toBe('Keyboard hint');
  });

  it('flips placement from top to bottom when top space is insufficient', () => {
    vi.spyOn(window, 'innerHeight', 'get').mockReturnValue(600);
    vi.spyOn(window, 'innerWidth', 'get').mockReturnValue(800);

    render(
      <Tooltip content="Edge hint" placement="top">
        <button type="button">TopEdge</button>
      </Tooltip>
    );

    const trigger = screen.getByRole('button', { name: 'TopEdge' });
    vi.spyOn(trigger, 'getBoundingClientRect').mockReturnValue({
      top: 4,
      bottom: 32,
      left: 100,
      right: 160,
      width: 60,
      height: 28,
      x: 100,
      y: 4,
      toJSON: () => ({}),
    });

    fireEvent.focus(trigger);
    const tooltip = screen.getByRole('tooltip');
    expect(tooltip.getAttribute('data-placement')).toBe('bottom');
  });

  it('supports disabled child triggers via wrapper when triggerDisabled is true', () => {
    render(
      <Tooltip content="Cannot delete active project" triggerDisabled>
        <button type="button" disabled>
          Delete
        </button>
      </Tooltip>
    );

    const btn = screen.getByRole('button', { name: 'Delete' }) as HTMLButtonElement;
    expect(btn.disabled).toBe(true);
    const wrapper = btn.parentElement as HTMLElement;
    expect(wrapper.className).toContain('tooltip-disabled-wrap');

    fireEvent.focus(wrapper);
    expect(screen.getByRole('tooltip').textContent).toBe('Cannot delete active project');
  });

  it('only opens when content overflows if onlyWhenTruncated is enabled', () => {
    render(
      <Tooltip content="Full file name.wav" onlyWhenTruncated>
        <div data-testid="trunc-target">Full file name.wav</div>
      </Tooltip>
    );

    const target = screen.getByTestId('trunc-target');
    Object.defineProperty(target, 'clientWidth', { configurable: true, value: 120 });
    Object.defineProperty(target, 'scrollWidth', { configurable: true, value: 120 });

    fireEvent.focus(target);
    expect(screen.queryByRole('tooltip')).toBeNull();

    Object.defineProperty(target, 'scrollWidth', { configurable: true, value: 240 });
    fireEvent.blur(target);
    fireEvent.focus(target);
    expect(screen.getByRole('tooltip').textContent).toBe('Full file name.wav');
  });
});
