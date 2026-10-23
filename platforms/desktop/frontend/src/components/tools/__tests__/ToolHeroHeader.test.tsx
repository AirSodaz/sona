import { fireEvent, render, screen } from '@testing-library/react';
import { Wrench } from 'lucide-react';
import { describe, expect, it, vi } from 'vitest';
import { ToolHeroHeader, type ToolHeroStatusTone } from '../ToolHeroHeader';

describe('ToolHeroHeader', () => {
  it('renders title, description, and icon', () => {
    render(
      <ToolHeroHeader
        id="test-tool-hero"
        icon={<Wrench data-testid="tool-icon" />}
        title="Test Tool"
        description="A great test tool description."
        statusText="Running"
        statusTone="active"
        enabled={true}
        onToggle={vi.fn()}
        switchAriaLabel="Toggle Test Tool"
      />
    );

    expect(screen.getByText('Test Tool')).toBeDefined();
    expect(screen.getByText('A great test tool description.')).toBeDefined();
    expect(screen.getByTestId('tool-icon')).toBeDefined();
    expect(screen.getByText('Running')).toBeDefined();
  });

  it.each<ToolHeroStatusTone>(['active', 'warning', 'inactive', 'error'])(
    'renders correctly with statusTone="%s"',
    (tone) => {
      const { container } = render(
        <ToolHeroHeader
          icon={<Wrench />}
          title="Tone Test"
          statusText={`Status ${tone}`}
          statusTone={tone}
          enabled={tone === 'active'}
          onToggle={vi.fn()}
          switchAriaLabel="Toggle Tone Test"
        />
      );

      const badge = container.querySelector(`.tools-status-${tone}`);
      expect(badge).not.toBeNull();
      expect(badge?.classList.contains('tools-status-badge')).toBe(true);
      expect(container.querySelector('.tools-status-dot')).not.toBeNull();
      expect(screen.getByText(`Status ${tone}`)).toBeDefined();
    }
  );

  it('triggers onToggle callback when Switch is toggled', () => {
    const handleToggle = vi.fn();
    render(
      <ToolHeroHeader
        icon={<Wrench />}
        title="Toggle Test"
        statusText="Stopped"
        statusTone="inactive"
        enabled={false}
        onToggle={handleToggle}
        switchAriaLabel="Toggle Switch"
      />
    );

    const switchEl = screen.getByRole('switch', { name: 'Toggle Switch' });
    expect(switchEl.getAttribute('aria-checked')).toBe('false');

    fireEvent.click(switchEl);
    expect(handleToggle).toHaveBeenCalledTimes(1);
    expect(handleToggle).toHaveBeenCalledWith(true);
  });

  it('renders actions slot when provided', () => {
    render(
      <ToolHeroHeader
        icon={<Wrench />}
        title="Actions Test"
        statusText="Ready"
        statusTone="active"
        enabled={true}
        onToggle={vi.fn()}
        switchAriaLabel="Toggle Actions"
        actions={<button type="button">Copy Endpoint</button>}
      />
    );

    const actionButton = screen.getByRole('button', { name: 'Copy Endpoint' });
    expect(actionButton).toBeDefined();
  });

  it('forwards disabled prop to Switch and prevents toggling', () => {
    const handleToggle = vi.fn();
    render(
      <ToolHeroHeader
        icon={<Wrench />}
        title="Disabled Test"
        statusText="Disabled"
        statusTone="inactive"
        enabled={false}
        disabled={true}
        onToggle={handleToggle}
        switchAriaLabel="Toggle Disabled"
      />
    );

    const switchEl = screen.getByRole('switch', { name: 'Toggle Disabled' });
    expect(switchEl.getAttribute('aria-disabled')).toBe('true');

    fireEvent.click(switchEl);
    expect(handleToggle).not.toHaveBeenCalled();
  });
});
