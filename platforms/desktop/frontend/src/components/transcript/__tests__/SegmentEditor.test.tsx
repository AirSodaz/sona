import { render, fireEvent, waitFor } from '@testing-library/react';
import { vi, describe, it, expect, beforeEach } from 'vitest';
import { SegmentEditor } from '../SegmentEditor';
import { getActiveEditor } from '../../../stores/transcriptRuntimeStore';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => key,
  }),
  initReactI18next: {
    type: '3rdParty',
    init: () => undefined,
  },
}));

describe('SegmentEditor', () => {
  let onSave: (html: string) => void;
  let onCancel: () => void;
  let onSplit: (leftHtml: string, rightHtml: string) => void;

  beforeEach(() => {
    onSave = vi.fn();
    onCancel = vi.fn();
    onSplit = vi.fn();
  });

  function renderEditor(initialHtml = 'Hello world') {
    const result = render(
      <SegmentEditor
        segmentId="seg-1"
        initialHtml={initialHtml}
        onSave={onSave}
        onCancel={onCancel}
        onSplit={onSplit}
      />,
    );
    const input = result.container.querySelector('[contenteditable="true"]') as HTMLDivElement;
    return { ...result, input };
  }

  it('renders a contenteditable element', () => {
    const { input } = renderEditor();
    expect(input).toBeTruthy();
    expect(input.getAttribute('contenteditable')).toBe('true');
  });

  it('calls onSave with HTML on Enter', async () => {
    const { input } = renderEditor();
    expect(input).toBeTruthy();

    fireEvent.keyDown(input, { key: 'Enter' });

    expect(onSave).toHaveBeenCalledOnce();
    const savedHtml = vi.mocked(onSave).mock.calls[0][0];
    expect(typeof savedHtml).toBe('string');
    expect(savedHtml.length).toBeGreaterThan(0);
  });

  it('calls onCancel on Escape', () => {
    const { input } = renderEditor();
    fireEvent.keyDown(input, { key: 'Escape' });
    expect(onCancel).toHaveBeenCalledOnce();
  });

  it('calls onSplit on Shift+Enter', async () => {
    const { input } = renderEditor();
    await waitFor(() => {
      expect(getActiveEditor()).toBeTruthy();
    });
    // Focus and set cursor at middle of text
    input.focus();
    const textSpan = input.querySelector('[data-lexical-text="true"]');
    const textNode = textSpan?.firstChild;
    if (textNode && textNode.nodeType === Node.TEXT_NODE && textNode.textContent && textNode.textContent.length > 5) {
      const range = document.createRange();
      range.setStart(textNode, 5);
      range.collapse(true);
      const sel = window.getSelection();
      sel?.removeAllRanges();
      sel?.addRange(range);
    }
    fireEvent.keyDown(input, { key: 'Enter', shiftKey: true });
    expect(onSplit).toHaveBeenCalledOnce();
  });

  it('calls onSave with HTML on blur', () => {
    const { input } = renderEditor();
    fireEvent.blur(input);
    expect(onSave).toHaveBeenCalledOnce();
    const savedHtml = vi.mocked(onSave).mock.calls[0][0];
    expect(typeof savedHtml).toBe('string');
    expect(savedHtml.length).toBeGreaterThan(0);
  });

  it('converts old format HTML on initialization', () => {
    const { input } = renderEditor('<b>Hello</b> <i>World</i>');
    expect(input).toBeTruthy();
  });

  it('does not call onSave for regular key presses', () => {
    const { input } = renderEditor();
    fireEvent.keyDown(input, { key: 'a' });
    fireEvent.keyDown(input, { key: 'b' });
    fireEvent.keyDown(input, { key: ' ' });
    expect(onSave).not.toHaveBeenCalled();
  });
});
