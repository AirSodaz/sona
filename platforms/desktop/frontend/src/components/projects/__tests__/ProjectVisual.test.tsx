import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import { ProjectBadge, ProjectVisual } from '../ProjectVisual';
import { ProjectCreateModal } from '../ProjectCreateModal';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, options?: { defaultValue?: string }) => options?.defaultValue ?? key,
    i18n: { language: 'en' },
  }),
}));

describe('ProjectVisual', () => {
  it('renders default folder icon with specified color', () => {
    const { container } = render(<ProjectVisual color="#10B981" />);
    const visual = container.querySelector('.project-visual');
    expect(visual).not.toBeNull();
    expect(visual?.classList.contains('project-visual--sm')).toBe(true);

    const svgWrap = container.querySelector('.project-visual-svg');
    expect(svgWrap?.getAttribute('style')).toContain('color: rgb(16, 185, 129)');
  });

  it('renders custom emoji icon', () => {
    render(<ProjectVisual icon="🚀" color="#F43F5E" />);
    const emoji = screen.getByText('🚀');
    expect(emoji).not.toBeNull();
    expect(emoji.classList.contains('project-visual-emoji')).toBe(true);
  });

  it('renders system icon with color', () => {
    const { container } = render(<ProjectVisual icon="system:mic" color="#8B5CF6" size="lg" showBackground />);
    const visual = container.querySelector('.project-visual');
    expect(visual?.classList.contains('project-visual--lg')).toBe(true);
    expect(visual?.classList.contains('project-visual--with-bg')).toBe(true);

    const svgWrap = container.querySelector('.project-visual-svg');
    expect(svgWrap?.getAttribute('style')).toContain('color: rgb(139, 92, 246)');
  });
});

describe('ProjectBadge', () => {
  it('renders project name and visual mark with CSS variable for contrast styling', () => {
    const { container } = render(
      <ProjectBadge name="Audio Docs" icon="🎙️" color="#6366F1" />
    );

    const badge = container.querySelector('.history-item-project-badge');
    expect(badge).not.toBeNull();
    expect(badge?.getAttribute('style')).toContain('--project-color: #6366F1');
    expect(screen.getByText('Audio Docs')).not.toBeNull();
    expect(screen.getByText('🎙️')).not.toBeNull();
  });
});

describe('ProjectCreateModal with unified identity selection', () => {
  it('renders both icon picker and color swatches and responds to changes', () => {
    const onNameChange = vi.fn();
    const onDescriptionChange = vi.fn();
    const onColorChange = vi.fn();
    const onIconChange = vi.fn();
    const onClose = vi.fn();
    const onCreate = vi.fn();

    render(
      <ProjectCreateModal
        isOpen
        name="New Work"
        description="A great project"
        color="#6366F1"
        icon="📁"
        onNameChange={onNameChange}
        onDescriptionChange={onDescriptionChange}
        onColorChange={onColorChange}
        onIconChange={onIconChange}
        onClose={onClose}
        onCreate={onCreate}
      />
    );

    // Name input is populated
    expect(screen.getByDisplayValue('New Work')).toBeDefined();

    // Icon button displays current icon
    const iconButton = screen.getByRole('button', { name: '📁' });
    expect(iconButton).toBeDefined();

    // Clicking icon button opens picker
    fireEvent.click(iconButton);

    // Clicking a color swatch inside picker calls onColorChange
    const emeraldSwatch = screen.getByLabelText('#10B981');
    expect(emeraldSwatch.getAttribute('data-tooltip')).toBe('#10B981');
    expect(emeraldSwatch.getAttribute('title')).toBeNull();
    fireEvent.click(emeraldSwatch);
    expect(onColorChange).toHaveBeenCalledWith('#10B981');
    // Selecting an emoji calls onIconChange
    const targetEmojiButton = screen.getByRole('button', { name: '🎯' });
    fireEvent.click(targetEmojiButton);
    expect(onIconChange).toHaveBeenCalledWith('🎯');
  });
});
