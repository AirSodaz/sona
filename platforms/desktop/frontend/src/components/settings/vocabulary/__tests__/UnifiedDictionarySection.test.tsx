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
    render(
      <UnifiedDictionarySection defaultMode="code" content={content} onUpdateContent={vi.fn()} />
    );

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
    render(
      <UnifiedDictionarySection
        defaultMode="code"
        content={invalidContent}
        onUpdateContent={vi.fn()}
      />
    );

    expect(screen.getByText(/Syntax & Link Issues/)).toBeDefined();
    expect(screen.getByText(/Multiple replacement arrows found/)).toBeDefined();
    expect(screen.getByText(/must end with a colon/)).toBeDefined();
  });

  it('suggests matching projects when typing under projects section', () => {
    const onUpdateContent = vi.fn();
    render(
      <UnifiedDictionarySection
        defaultMode="code"
        content={'projects:\n'}
        onUpdateContent={onUpdateContent}
      />
    );

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
    render(
      <UnifiedDictionarySection
        defaultMode="code"
        content={'projects:\n'}
        onUpdateContent={onUpdateContent}
      />
    );

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

  it('renders visual mode by default with hotwords chips and replacement rules', () => {
    const content = `
- Sona
- Sherpa-onnx :2.0
- 苦伯内提斯 -> Kubernetes

projects:
  Weekly Review (id:proj-1):
    - Sprint Planning
`;
    render(<UnifiedDictionarySection content={content} onUpdateContent={vi.fn()} />);

    // Default view is visual mode
    expect(screen.getByText('Hotwords & Vocabulary')).toBeDefined();
    expect(screen.getByText('Text Replacements & Corrections')).toBeDefined();

    // Hotword chips
    expect(screen.getByText('Sona')).toBeDefined();
    expect(screen.getByText('Sherpa-onnx')).toBeDefined();
    expect(screen.getByText('x2.0')).toBeDefined();
    expect(screen.getByText('Sprint Planning')).toBeDefined();

    // Replacements
    expect(screen.getByText('苦伯内提斯')).toBeDefined();
    expect(screen.getByText('Kubernetes')).toBeDefined();
  });

  it('filters terms by scope and search in visual mode', () => {
    const content = `
- Sona
projects:
  Voice Typing (id:voice-typing):
    - VoiceWord
  Weekly Review (id:proj-1):
    - ProjectWord
`;
    render(<UnifiedDictionarySection content={content} onUpdateContent={vi.fn()} />);

    // Filter by Voice Typing scope
    const vtPill = screen.getByRole('button', { name: /Voice Typing/ });
    fireEvent.click(vtPill);

    expect(screen.getByText('VoiceWord')).toBeDefined();
    expect(screen.queryByText('ProjectWord')).toBeNull();
    expect(screen.queryByText('Sona')).toBeNull();

    // Search box
    const searchInput = screen.getByPlaceholderText(/Search hotwords or replacements/);
    fireEvent.change(searchInput, { target: { value: 'Voice' } });
    expect(screen.getByText('VoiceWord')).toBeDefined();

    fireEvent.change(searchInput, { target: { value: 'NonExistent' } });
    expect(screen.queryByText('VoiceWord')).toBeNull();
  });

  it('deletes a hotword chip in visual mode', () => {
    const onUpdateContent = vi.fn();
    const content = `- Sona\n- DeepSeek\n`;

    render(<UnifiedDictionarySection content={content} onUpdateContent={onUpdateContent} />);

    const deleteBtn = screen.getByRole('button', { name: 'Delete DeepSeek' });
    fireEvent.click(deleteBtn);

    expect(onUpdateContent).toHaveBeenCalledTimes(1);
    const updated = onUpdateContent.mock.calls[0][0];
    expect(updated).not.toContain('- DeepSeek');
    expect(updated).toContain('- Sona');
  });

  it('deletes a replacement rule in visual mode', () => {
    const onUpdateContent = vi.fn();
    const content = `- 苦伯内提斯 -> Kubernetes\n`;

    render(<UnifiedDictionarySection content={content} onUpdateContent={onUpdateContent} />);

    const deleteBtn = screen.getByRole('button', { name: 'Delete 苦伯内提斯' });
    fireEvent.click(deleteBtn);

    expect(onUpdateContent).toHaveBeenCalledTimes(1);
    const updated = onUpdateContent.mock.calls[0][0];
    expect(updated).not.toContain('苦伯内提斯 -> Kubernetes');
  });

  it('adds term via structured new entry modal in visual mode', () => {
    const onUpdateContent = vi.fn();
    const content = `- Sona\n`;

    render(<UnifiedDictionarySection content={content} onUpdateContent={onUpdateContent} />);

    // Open modal
    fireEvent.click(screen.getByRole('button', { name: 'New Entry' }));
    expect(screen.getByText('New Vocabulary Entry')).toBeDefined();

    // Fill hotword with weight
    const wordInput = screen.getByPlaceholderText(/e\.g\. Sona or DeepSeek/);
    fireEvent.change(wordInput, { target: { value: 'Sherpa-onnx' } });

    const weightInput = screen.getByPlaceholderText('e.g. 2.0');
    fireEvent.change(weightInput, { target: { value: '2.0' } });

    fireEvent.click(screen.getByRole('button', { name: 'Save Entry' }));

    expect(onUpdateContent).toHaveBeenCalledTimes(1);
    const updated = onUpdateContent.mock.calls[0][0];
    expect(updated).toContain('- Sherpa-onnx :2.0');
  });

  it('adds replacement rule via structured new entry modal', () => {
    const onUpdateContent = vi.fn();
    const content = `- Sona\n`;

    render(<UnifiedDictionarySection content={content} onUpdateContent={onUpdateContent} />);

    // Open modal
    fireEvent.click(screen.getByRole('button', { name: 'New Entry' }));

    // Switch to replacement type
    fireEvent.click(screen.getByRole('button', { name: /Replacement Rule/ }));

    const fromInput = screen.getByPlaceholderText(/Misrecognized word/);
    const toInput = screen.getByPlaceholderText(/Corrected word/);

    fireEvent.change(fromInput, { target: { value: '微信支付' } });
    fireEvent.change(toInput, { target: { value: 'WeChat Pay' } });

    fireEvent.click(screen.getByRole('button', { name: 'Save Entry' }));

    expect(onUpdateContent).toHaveBeenCalledTimes(1);
    const updated = onUpdateContent.mock.calls[0][0];
    expect(updated).toContain('- 微信支付 -> WeChat Pay');
  });

  it('adds hotword via inline quick input in visual mode', () => {
    const onUpdateContent = vi.fn();
    const content = `- Sona\n`;

    render(<UnifiedDictionarySection content={content} onUpdateContent={onUpdateContent} />);

    const inlineInput = screen.getByPlaceholderText('+ Add hotword...');
    fireEvent.change(inlineInput, { target: { value: 'InlineWord' } });
    fireEvent.keyDown(inlineInput, { key: 'Enter', code: 'Enter' });

    expect(onUpdateContent).toHaveBeenCalledTimes(1);
    const updated = onUpdateContent.mock.calls[0][0];
    expect(updated).toContain('- InlineWord');
  });

  it('tests replacement rules with live preview in visual mode', () => {
    const content = `- 苦伯内提斯 -> Kubernetes\n`;
    render(<UnifiedDictionarySection content={content} onUpdateContent={vi.fn()} />);

    const testInput = screen.getByPlaceholderText(/Type or paste text to test replacement/);
    fireEvent.change(testInput, { target: { value: '使用苦伯内提斯部署' } });

    expect(screen.getByText('Result:')).toBeDefined();
    expect(screen.getByText('使用Kubernetes部署')).toBeDefined();
  });

  it('switches between visual and code mode', () => {
    const content = `- Sona\n`;
    render(<UnifiedDictionarySection content={content} onUpdateContent={vi.fn()} />);

    // Initially visual
    expect(screen.getByRole('tab', { name: /Visual/ }).getAttribute('aria-selected')).toBe('true');
    expect(screen.getByText('Hotwords & Vocabulary')).toBeDefined();

    // Switch to code mode
    fireEvent.click(screen.getByRole('tab', { name: /YAML Code/ }));
    expect(screen.getByRole('tab', { name: /YAML Code/ }).getAttribute('aria-selected')).toBe(
      'true'
    );
    expect(screen.getByLabelText('Unified Dictionary Editor')).toBeDefined();

    // Switch back to visual mode
    fireEvent.click(screen.getByRole('tab', { name: /Visual/ }));
    expect(screen.getByRole('tab', { name: /Visual/ }).getAttribute('aria-selected')).toBe('true');
    expect(screen.getByText('Hotwords & Vocabulary')).toBeDefined();
  });
});
