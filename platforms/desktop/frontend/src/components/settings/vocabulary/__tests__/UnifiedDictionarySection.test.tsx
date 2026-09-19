import { fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useProjectStore } from '../../../../stores/projectStore';
import { UnifiedDictionarySection } from '../UnifiedDictionarySection';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, options?: { defaultValue?: string }) => options?.defaultValue ?? key,
    i18n: { language: 'en' },
  }),
}));

describe('UnifiedDictionarySection', () => {
  beforeEach(() => {
    useProjectStore.setState({
      projects: [
        {
          id: 'proj-1',
          name: 'Weekly Review',
          description: '',
          icon: '',
          sortOrder: 0,
          createdAt: 0,
          updatedAt: 0,
        },
      ],
    });
  });

  it('renders YAML dictionary editor and statistics correctly', () => {
    const content = `
# Global Terms
- Sona
- 苦伯内提斯 -> Kubernetes

projects:
  Weekly Review (id:proj-1):
    - Sprint Planning
`;
    render(<UnifiedDictionarySection content={content} onUpdateContent={vi.fn()} />);

    expect(screen.getByText('Unified Dictionary')).toBeDefined();
    expect(screen.getByText(/Total terms:/)).toBeDefined();
    expect(screen.getByText(/Global:/)).toBeDefined();
    expect(screen.getByText(/Projects:/)).toBeDefined();

    const editor = screen.getByLabelText('Unified Dictionary Editor') as HTMLTextAreaElement;
    expect(editor.value).toContain('Weekly Review (id:proj-1):');
  });

  it('adds a new term to global section via quick add input', () => {
    const onUpdateContent = vi.fn();
    const content = '- Sona\n';

    render(<UnifiedDictionarySection content={content} onUpdateContent={onUpdateContent} />);

    const input = screen.getByPlaceholderText(/Term.*or From -> To/);
    fireEvent.change(input, { target: { value: 'DeepSeek' } });
    fireEvent.click(screen.getByRole('button', { name: 'Add' }));

    expect(onUpdateContent).toHaveBeenCalledTimes(1);
    const updated = onUpdateContent.mock.calls[0][0];
    expect(updated).toContain('- DeepSeek');
  });

  it('formats dictionary content and updates indentation', () => {
    const onUpdateContent = vi.fn();
    const messyContent = `
- Sona
projects:
 Weekly Review:
  - Sprint
`;
    render(<UnifiedDictionarySection content={messyContent} onUpdateContent={onUpdateContent} />);

    const formatBtn = screen.getByRole('button', { name: 'Format' });
    expect(formatBtn.getAttribute('data-tooltip')).toBe(
      'Format indentation, canonicalize arrows, and sync project links'
    );
    expect(formatBtn.getAttribute('data-tooltip-pos')).toBe('top');
    expect(formatBtn.getAttribute('title')).toBeNull();

    const copyBtn = screen.getByRole('button', { name: 'Copy dictionary YAML to clipboard' });
    expect(copyBtn.getAttribute('data-tooltip')).toBe('Copy dictionary YAML to clipboard');
    expect(copyBtn.getAttribute('data-tooltip-pos')).toBe('top');
    expect(copyBtn.getAttribute('title')).toBeNull();

    fireEvent.click(formatBtn);
    expect(onUpdateContent).toHaveBeenCalledTimes(1);
    const formatted = onUpdateContent.mock.calls[0][0];
    expect(formatted).toContain('  Weekly Review (id:proj-1):');
    expect(formatted).toContain('    - Sprint');
  });

  it('displays syntax diagnostics when invalid lines or multiple arrows exist', () => {
    const invalidContent = `
- Sona -> Another -> Third
projects:
  MissingColon
    - Item
`;
    render(<UnifiedDictionarySection content={invalidContent} onUpdateContent={vi.fn()} />);

    expect(screen.getByText(/Syntax & Link Issues/)).toBeDefined();
    expect(screen.getByText(/Multiple replacement arrows found/)).toBeDefined();
    expect(screen.getByText(/must end with a colon/)).toBeDefined();
  });

  it('suggests matching projects when typing under projects section', () => {
    const onUpdateContent = vi.fn();
    render(<UnifiedDictionarySection content={'projects:\n'} onUpdateContent={onUpdateContent} />);

    const editor = screen.getByLabelText('Unified Dictionary Editor');
    fireEvent.change(editor, {
      target: {
        value: 'projects:\n  Week',
      },
    });

    expect(screen.getByText('Link Project')).toBeDefined();
    const suggestionBtn = screen.getByRole('button', { name: /Weekly Review/ });
    expect(suggestionBtn).toBeDefined();

    fireEvent.click(suggestionBtn);
    const updated = onUpdateContent.mock.calls[onUpdateContent.mock.calls.length - 1][0];
    expect(updated).toContain('Weekly Review (id:proj-1):');
  });

  it('adds a new term to voice typing scope via quick add target dropdown', () => {
    const onUpdateContent = vi.fn();
    const content = '- Sona\n';

    render(<UnifiedDictionarySection content={content} onUpdateContent={onUpdateContent} />);

    // Open scope dropdown and select voice-typing
    const scopeTrigger = screen.getByLabelText('Target:');
    fireEvent.click(scopeTrigger);
    const vtOption = screen.getByRole('option', { name: /Voice Typing/ });
    fireEvent.click(vtOption);
    const input = screen.getByPlaceholderText(/Term.*or From -> To/);
    fireEvent.change(input, { target: { value: 'DictationWord' } });
    fireEvent.click(screen.getByRole('button', { name: 'Add' }));

    expect(onUpdateContent).toHaveBeenCalledTimes(1);
    const updated = onUpdateContent.mock.calls[0][0];
    expect(updated).toContain('Voice Typing (id:voice-typing):');
    expect(updated).toContain('- DictationWord');
  });

  it('suggests voice typing scope when typing under projects section', () => {
    const onUpdateContent = vi.fn();
    render(<UnifiedDictionarySection content={'projects:\n'} onUpdateContent={onUpdateContent} />);

    const editor = screen.getByLabelText('Unified Dictionary Editor');
    fireEvent.change(editor, {
      target: {
        value: 'projects:\n  Voice',
      },
    });

    expect(screen.getByText('Link Project')).toBeDefined();
    const suggestionBtn = screen.getByRole('button', { name: /Voice Typing/ });
    expect(suggestionBtn).toBeDefined();

    fireEvent.click(suggestionBtn);
    const updated = onUpdateContent.mock.calls[onUpdateContent.mock.calls.length - 1][0];
    expect(updated).toContain('Voice Typing (id:voice-typing):');
  });
});
