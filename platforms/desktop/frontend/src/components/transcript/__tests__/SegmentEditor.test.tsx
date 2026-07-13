import { act, render, fireEvent, screen, waitFor } from '@testing-library/react';
import { StrictMode, useState } from 'react';
import { vi, describe, it, expect, beforeEach } from 'vitest';
import {
  $getRoot,
  $getSelection,
  $isRangeSelection,
  $isTextNode,
  FORMAT_TEXT_COMMAND,
  SELECT_ALL_COMMAND,
} from 'lexical';
import { SegmentEditor } from '../SegmentEditor';
import { getActiveEditor } from '../../../stores/transcriptRuntimeStore';
import { ContextMenuProvider } from '../../context-menu/ContextMenuProvider';
import { useContextMenu } from '../../context-menu/useContextMenu';

const loggerErrorMock = vi.hoisted(() => vi.fn());

vi.mock('../../../utils/logger', () => ({
  logger: {
    error: loggerErrorMock,
  },
}));

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
    loggerErrorMock.mockReset();
    Object.defineProperty(Range.prototype, 'getBoundingClientRect', {
      configurable: true,
      value: vi.fn(() => new DOMRect()),
    });
    onSave = vi.fn();
    onCancel = vi.fn();
    onSplit = vi.fn();
    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: {
        readText: vi.fn().mockResolvedValue(' pasted'),
        writeText: vi.fn().mockResolvedValue(undefined),
      },
    });
  });

  function ActiveContext(): React.JSX.Element {
    const { activeContextId } = useContextMenu();
    return <output aria-label="Active context">{activeContextId ?? 'none'}</output>;
  }

  function ReplacementMenuButton({ onReplace }: { onReplace: () => void }): React.JSX.Element {
    const { openContextMenu } = useContextMenu();
    return (
      <button
        type="button"
        onClick={(event) => {
          openContextMenu({
            contextId: 'replacement-menu',
            ariaLabel: 'Replacement menu',
            actions: [{ id: 'replacement', label: 'Replacement', onSelect: vi.fn() }],
            anchor: event.currentTarget,
            point: { x: 40, y: 40 },
            invocation: 'pointer',
          });
          onReplace();
        }}
      >
        Replace editor menu
      </button>
    );
  }

  function renderEditor(initialHtml = 'Hello world', strict = false) {
    const editor = (
      <ContextMenuProvider>
        <SegmentEditor
          segmentId="seg-1"
          initialHtml={initialHtml}
          onSave={onSave}
          onCancel={onCancel}
          onSplit={onSplit}
        />
        <ActiveContext />
      </ContextMenuProvider>
    );
    const result = render(
      strict ? <StrictMode>{editor}</StrictMode> : editor,
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

  it('opens the editing menu from pointer and keyboard context-menu triggers', async () => {
    const { input } = renderEditor();

    fireEvent.contextMenu(input, { clientX: 32, clientY: 48 });
    expect(screen.getByLabelText('Active context').textContent).toBe('editor:editing:seg-1');
    expect(screen.getByRole('menu', { name: 'editor.context_menu_label' })).toBeDefined();
    expect((screen.getByRole('menuitem', { name: 'common.cut' }) as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByRole('menuitem', { name: 'common.copy' }) as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByRole('menuitem', { name: 'common.paste' }) as HTMLButtonElement).disabled).toBe(false);

    fireEvent.keyDown(screen.getByRole('menu'), { key: 'Escape' });
    fireEvent.keyDown(input, { key: 'F10', shiftKey: true });
    expect(screen.getByRole('menu')).toBeDefined();
    fireEvent.keyDown(screen.getByRole('menu'), { key: 'Escape' });
    fireEvent.keyDown(input, { key: 'ContextMenu' });
    expect(screen.getByRole('menu')).toBeDefined();
  });

  it('copies and cuts the captured selection as plain text', async () => {
    const { input } = renderEditor();
    const editor = await waitFor(() => {
      const activeEditor = getActiveEditor();
      expect(activeEditor).toBeTruthy();
      return activeEditor!;
    });
    await act(async () => {
      await new Promise<void>((resolve) => editor.update(() => {
        const text = $getRoot().getFirstDescendant();
        if ($isTextNode(text)) text.select(0, 5);
      }, { onUpdate: resolve }));
    });

    fireEvent.contextMenu(input, { clientX: 12, clientY: 12 });
    fireEvent.click(screen.getByRole('menuitem', { name: 'common.copy' }));
    await waitFor(() => expect(navigator.clipboard.writeText).toHaveBeenCalledWith('Hello'));

    await act(async () => {
      await new Promise<void>((resolve) => editor.update(() => {
        const text = $getRoot().getFirstDescendant();
        if ($isTextNode(text)) text.select(0, 5);
      }, { onUpdate: resolve }));
    });
    fireEvent.contextMenu(input, { clientX: 12, clientY: 12 });
    fireEvent.click(screen.getByRole('menuitem', { name: 'common.cut' }));
    await waitFor(() => expect(input.textContent).toBe(' world'));
  });

  it('pastes plain text and exposes select and formatting actions', async () => {
    const { input } = renderEditor();
    const editor = await waitFor(() => getActiveEditor()!);
    await act(async () => {
      await new Promise<void>((resolve) => editor.update(() => {
        const text = $getRoot().getFirstDescendant();
        if ($isTextNode(text)) text.selectEnd();
      }, { onUpdate: resolve }));
    });

    fireEvent.contextMenu(input, { clientX: 12, clientY: 12 });
    expect((screen.getByRole('menuitem', { name: 'common.select_all' }) as HTMLButtonElement).disabled).toBe(false);
    expect((screen.getByRole('menuitem', { name: 'editor.bold' }) as HTMLButtonElement).disabled).toBe(false);
    expect((screen.getByRole('menuitem', { name: 'editor.italic' }) as HTMLButtonElement).disabled).toBe(false);
    expect((screen.getByRole('menuitem', { name: 'editor.underline' }) as HTMLButtonElement).disabled).toBe(false);
    fireEvent.click(screen.getByRole('menuitem', { name: 'common.paste' }));

    await waitFor(() => expect(input.textContent).toBe('Hello world pasted'));
  });

  it('dispatches select-all and formatting commands after restoring editor focus', async () => {
    const { input } = renderEditor();
    const editor = await waitFor(() => getActiveEditor()!);
    const focusSpy = vi.spyOn(input, 'focus');
    const dispatchSpy = vi.spyOn(editor, 'dispatchCommand');

    fireEvent.contextMenu(input, { clientX: 12, clientY: 12 });
    fireEvent.click(screen.getByRole('menuitem', { name: 'common.select_all' }));
    await waitFor(() => expect(dispatchSpy).toHaveBeenCalledWith(
      SELECT_ALL_COMMAND,
      expect.any(KeyboardEvent),
    ));
    expect(focusSpy).toHaveBeenCalled();
    await waitFor(() => editor.getEditorState().read(() => {
      expect($getSelection()?.getTextContent()).toBe('Hello world');
    }));

    for (const format of ['bold', 'italic', 'underline'] as const) {
      fireEvent.contextMenu(input, { clientX: 12, clientY: 12 });
      fireEvent.click(screen.getByRole('menuitem', { name: `editor.${format}` }));
      await waitFor(() => expect(dispatchSpy).toHaveBeenCalledWith(FORMAT_TEXT_COMMAND, format));
      await waitFor(() => editor.getEditorState().read(() => {
        const selection = $getSelection();
        expect($isRangeSelection(selection) && selection.hasFormat(format)).toBe(true);
      }));
    }
  });

  it('keeps text unchanged and logs when cutting cannot write to the clipboard', async () => {
    const cutError = new Error('clipboard denied');
    vi.mocked(navigator.clipboard.writeText).mockRejectedValueOnce(cutError);
    const { input } = renderEditor();
    const editor = await waitFor(() => getActiveEditor()!);
    await act(async () => {
      await new Promise<void>((resolve) => editor.update(() => {
        const text = $getRoot().getFirstDescendant();
        if ($isTextNode(text)) text.select(0, 5);
      }, { onUpdate: resolve }));
    });

    fireEvent.contextMenu(input, { clientX: 12, clientY: 12 });
    fireEvent.click(screen.getByRole('menuitem', { name: 'common.cut' }));

    await waitFor(() => expect(loggerErrorMock).toHaveBeenCalledWith(
      '[SegmentEditorContextMenu] Failed to cut text:',
      cutError,
    ));
    expect(input.textContent).toBe('Hello world');
  });

  it('keeps asynchronous paste usable under StrictMode effect replay', async () => {
    const { input } = renderEditor('Hello world', true);
    const editor = await waitFor(() => {
      const activeEditor = getActiveEditor();
      expect(activeEditor).toBeTruthy();
      return activeEditor!;
    });
    await act(async () => {
      await new Promise<void>((resolve) => editor.update(() => {
        const text = $getRoot().getFirstDescendant();
        if ($isTextNode(text)) text.selectEnd();
      }, { onUpdate: resolve }));
    });

    fireEvent.contextMenu(input, { clientX: 12, clientY: 12 });
    fireEvent.click(screen.getByRole('menuitem', { name: 'common.paste' }));

    await waitFor(() => expect(input.textContent).toBe('Hello world pasted'));
  });

  it('pastes at the captured selection when a newer menu changes the live selection', async () => {
    let resolveRead!: (text: string) => void;
    vi.mocked(navigator.clipboard.readText).mockReturnValue(new Promise((resolve) => {
      resolveRead = resolve;
    }));
    const { input } = renderEditor();
    const editor = await waitFor(() => getActiveEditor()!);
    await act(async () => {
      await new Promise<void>((resolve) => editor.update(() => {
        const text = $getRoot().getFirstDescendant();
        if ($isTextNode(text)) text.selectEnd();
      }, { onUpdate: resolve }));
    });

    fireEvent.contextMenu(input, { clientX: 12, clientY: 12 });
    fireEvent.click(screen.getByRole('menuitem', { name: 'common.paste' }));
    await act(async () => {
      await Promise.resolve();
    });
    await act(async () => {
      await new Promise<void>((resolve) => editor.update(() => {
        const text = $getRoot().getFirstDescendant();
        if ($isTextNode(text)) text.selectStart();
      }, { onUpdate: resolve }));
    });
    fireEvent.contextMenu(input, { clientX: 24, clientY: 24 });

    resolveRead(' captured');
    await waitFor(() => expect(input.textContent).toBe('Hello world captured'));
  });

  it('restores editor focus when reading the clipboard fails', async () => {
    const pasteError = new Error('clipboard denied');
    vi.mocked(navigator.clipboard.readText).mockRejectedValueOnce(pasteError);
    const { input } = renderEditor();

    fireEvent.contextMenu(input, { clientX: 12, clientY: 12 });
    fireEvent.click(screen.getByRole('menuitem', { name: 'common.paste' }));

    await waitFor(() => expect(loggerErrorMock).toHaveBeenCalledWith(
      '[SegmentEditorContextMenu] Failed to paste text:',
      pasteError,
    ));
    await waitFor(() => expect(document.activeElement).toBe(input));
  });

  it('closes an open menu when the editor unmounts without committing stale content', () => {
    const result = renderEditor();
    fireEvent.contextMenu(result.input, { clientX: 12, clientY: 12 });
    expect(screen.getByRole('menu')).toBeDefined();

    result.rerender(
      <ContextMenuProvider>
        <ActiveContext />
      </ContextMenuProvider>,
    );

    expect(screen.queryByRole('menu')).toBeNull();
    expect(screen.getByLabelText('Active context').textContent).toBe('none');
    expect(onSave).not.toHaveBeenCalled();
  });

  it('does not close a replacement menu when the previous editor unmounts in the same batch', () => {
    function ReplacementHarness(): React.JSX.Element {
      const [showEditor, setShowEditor] = useState(true);
      return (
        <ContextMenuProvider>
          {showEditor && (
            <SegmentEditor
              segmentId="seg-1"
              initialHtml="Hello world"
              onSave={onSave}
              onCancel={onCancel}
              onSplit={onSplit}
            />
          )}
          <ReplacementMenuButton onReplace={() => setShowEditor(false)} />
          <ActiveContext />
        </ContextMenuProvider>
      );
    }

    const { container } = render(<ReplacementHarness />);
    const input = container.querySelector('[contenteditable="true"]') as HTMLDivElement;
    fireEvent.contextMenu(input, { clientX: 12, clientY: 12 });

    fireEvent.click(screen.getByRole('button', { name: 'Replace editor menu' }));

    expect(screen.getByRole('menu', { name: 'Replacement menu' })).toBeDefined();
    expect(screen.getByLabelText('Active context').textContent).toBe('replacement-menu');
  });

  it('suppresses blur saving while the menu is open and commits on outside close', () => {
    const { input } = renderEditor();
    fireEvent.contextMenu(input, { clientX: 12, clientY: 12 });
    fireEvent.blur(input);
    expect(onSave).not.toHaveBeenCalled();

    fireEvent.keyDown(screen.getByRole('menu'), { key: 'Escape' });
    expect(onSave).not.toHaveBeenCalled();

    fireEvent.contextMenu(input, { clientX: 12, clientY: 12 });
    fireEvent.pointerDown(document.body);
    expect(onSave).toHaveBeenCalledOnce();
  });

  it('does not focus or mutate after an asynchronous paste finishes post-unmount', async () => {
    let resolveRead!: (text: string) => void;
    vi.mocked(navigator.clipboard.readText).mockReturnValue(new Promise((resolve) => {
      resolveRead = resolve;
    }));
    const { input, unmount } = renderEditor();
    const editor = await waitFor(() => getActiveEditor()!);
    const focusSpy = vi.spyOn(input, 'focus');
    const dispatchSpy = vi.spyOn(editor, 'dispatchCommand');

    fireEvent.contextMenu(input, { clientX: 12, clientY: 12 });
    fireEvent.click(screen.getByRole('menuitem', { name: 'common.paste' }));
    unmount();
    focusSpy.mockClear();
    dispatchSpy.mockClear();
    resolveRead('late paste');
    await Promise.resolve();
    await Promise.resolve();

    expect(focusSpy).not.toHaveBeenCalled();
    expect(dispatchSpy).not.toHaveBeenCalled();
  });
});
