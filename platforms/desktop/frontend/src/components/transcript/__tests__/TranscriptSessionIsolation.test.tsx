import { act, fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { createStore } from 'zustand/vanilla';
import { TranscriptSessionProvider } from '../../../contexts/TranscriptSessionContext';
import { usePolishActions } from '../../../hooks/usePolishActions';
import {
  addLlmModel,
  createLlmSettings,
  setFeatureModelSelection,
  updateProviderSetting,
} from '../../../services/llm/state';
import { polishService } from '../../../services/polishService';
import { useConfigStore } from '../../../stores/configStore';
import { useEffectiveConfigStore } from '../../../stores/effectiveConfigStore';
import { useTranscriptSessionStore } from '../../../stores/transcriptSessionStore';
import { buildTestConfig } from '../../../test-utils/configTestUtils';
import {
  resetTranscriptStores,
  useTranscriptStore,
} from '../../../test-utils/transcriptStoreTestUtils';
import { normalizeTranscriptSegment } from '../../../utils/transcriptTiming';
import { ContextMenuProvider } from '../../context-menu/ContextMenuProvider';
import { SegmentItem } from '../SegmentItem';
import { TranscriptUIContext, type TranscriptUIState } from '../TranscriptUIContext';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => key,
  }),
  initReactI18next: {
    type: '3rdParty',
    init: () => undefined,
  },
}));

vi.mock('../../Icons', () => ({
  EditIcon: () => <span data-testid="edit-icon" />,
  TrashIcon: () => <span data-testid="trash-icon" />,
  MergeIcon: () => <span data-testid="merge-icon" />,
  CheckIcon: () => <span data-testid="check-icon" />,
  CloseIcon: () => <span data-testid="close-icon" />,
}));

vi.mock('../SegmentTimestamp', () => ({
  SegmentTimestamp: ({ start }: { start: number }) => <span>{start}</span>,
}));

function ScopedSessionHarness({
  prefix,
  onReady,
}: {
  prefix: string;
  onReady: (api: ReturnType<typeof usePolishActions>) => void;
}) {
  const segments = useTranscriptSessionStore((state) => state.segments);
  const actions = usePolishActions();
  onReady(actions);
  return <div data-testid={`${prefix}-text`}>{segments.map((s) => s.text).join('|')}</div>;
}

describe('TranscriptSessionIsolation & Document Read-Only Lock', () => {
  beforeEach(() => {
    resetTranscriptStores();
    vi.restoreAllMocks();
  });

  it('keeps mounted Create and Projects session views and usePolishActions undo stacks isolated', async () => {
    let llmSettings = createLlmSettings('open_ai');
    llmSettings = updateProviderSetting(llmSettings, 'open_ai', {
      apiHost: 'https://api.openai.com',
      apiKey: 'test-key',
    });
    llmSettings = addLlmModel(llmSettings, { provider: 'open_ai', model: 'gpt-4o-mini' });
    llmSettings = setFeatureModelSelection(llmSettings, 'polish', llmSettings.modelOrder[0]);
    const readyConfig = buildTestConfig({ llmSettings });
    useConfigStore.setState({ config: readyConfig });
    useEffectiveConfigStore.setState({ config: readyConfig });

    vi.spyOn(polishService, 'polishTranscript').mockImplementation(async (options) => {
      const targetSessionId = options?.sessionId ?? useTranscriptStore.getState().activeSessionId;
      useTranscriptStore.getState().setSegmentsForSession(targetSessionId, [
        normalizeTranscriptSegment({
          id: targetSessionId === 'history-a' ? 'p-1' : 'c-1',
          start: 0,
          end: 1,
          text: targetSessionId === 'history-a' ? 'project-v2' : 'create-v2',
          isFinal: true,
        }),
      ]);
    });

    useTranscriptStore.setState({
      mode: 'projects',
      activeSessionId: 'history-a',
      createLiveSessionId: 'default',
      sessions: {
        default: {
          ...useTranscriptStore.getState().sessions.default,
          sourceHistoryId: null,
          segments: [
            normalizeTranscriptSegment({
              id: 'c-1',
              start: 0,
              end: 1,
              text: 'create-v1',
              isFinal: true,
            }),
          ],
        },
        'history-a': {
          ...useTranscriptStore.getState().sessions.default,
          sourceHistoryId: 'history-a',
          segments: [
            normalizeTranscriptSegment({
              id: 'p-1',
              start: 0,
              end: 1,
              text: 'project-v1',
              isFinal: true,
            }),
          ],
        },
      },
    });

    let createActions!: ReturnType<typeof usePolishActions>;
    let projectActions!: ReturnType<typeof usePolishActions>;

    render(
      <>
        <TranscriptSessionProvider sessionId="default" isActive={false}>
          <ScopedSessionHarness prefix="create" onReady={(api) => (createActions = api)} />
        </TranscriptSessionProvider>
        <TranscriptSessionProvider sessionId="history-a" isActive={true}>
          <ScopedSessionHarness prefix="project" onReady={(api) => (projectActions = api)} />
        </TranscriptSessionProvider>
      </>
    );

    expect(screen.getByTestId('create-text').textContent).toBe('create-v1');
    expect(screen.getByTestId('project-text').textContent).toBe('project-v1');

    await act(async () => {
      await projectActions.handleStartPolish();
    });

    expect(screen.getByTestId('project-text').textContent).toBe('project-v2');
    expect(screen.getByTestId('create-text').textContent).toBe('create-v1');
    expect(projectActions.undoSegments).not.toBeNull();
    expect(createActions.undoSegments).toBeNull();

    await act(async () => {
      await createActions.handleStartPolish();
    });
    expect(screen.getByTestId('create-text').textContent).toBe('create-v2');

    // Undo in Projects leaves Create untouched and vice versa
    act(() => {
      projectActions.handleUndoPolish();
    });
    expect(screen.getByTestId('project-text').textContent).toBe('project-v1');
    expect(screen.getByTestId('create-text').textContent).toBe('create-v2');

    act(() => {
      createActions.handleUndoPolish();
    });
    expect(screen.getByTestId('create-text').textContent).toBe('create-v1');
    expect(screen.getByTestId('project-text').textContent).toBe('project-v1');

    act(() => {
      projectActions.handleRedoPolish();
    });
    expect(screen.getByTestId('project-text').textContent).toBe('project-v2');
    expect(screen.getByTestId('create-text').textContent).toBe('create-v1');
  });

  it('disables inline edit, merge, and delete controls on SegmentItem while isDocumentLocked is true', () => {
    const onEdit = vi.fn();
    const onDelete = vi.fn();
    const onMergeWithNext = vi.fn();
    const uiStore = createStore<TranscriptUIState>(() => ({
      newSegmentIds: new Set(),
      activeSegmentId: null,
      editingSegmentId: null,
      totalSegments: 2,
      aligningSegmentIds: new Set(),
      isDocumentLocked: true,
    }));

    const segment = normalizeTranscriptSegment({
      id: 'locked-seg',
      start: 0,
      end: 3,
      text: 'Locked transcript line',
      isFinal: true,
    });

    render(
      <ContextMenuProvider>
        <TranscriptUIContext.Provider value={uiStore}>
          <SegmentItem
            segment={segment}
            index={0}
            onSeek={vi.fn()}
            onEdit={onEdit}
            onSave={vi.fn()}
            onDelete={onDelete}
            onMergeWithNext={onMergeWithNext}
            onAnimationEnd={vi.fn()}
          />
        </TranscriptUIContext.Provider>
      </ContextMenuProvider>
    );

    const editBtn = screen.getByRole('button', {
      name: /editor\.edit_label/i,
    }) as HTMLButtonElement;
    const mergeBtn = screen.getByRole('button', {
      name: /editor\.merge_label/i,
    }) as HTMLButtonElement;
    const deleteBtn = screen.getByRole('button', {
      name: /editor\.delete_label/i,
    }) as HTMLButtonElement;

    expect(editBtn.disabled).toBe(true);
    expect(mergeBtn.disabled).toBe(true);
    expect(deleteBtn.disabled).toBe(true);

    fireEvent.click(editBtn);
    fireEvent.doubleClick(screen.getByText('Locked transcript line'));
    expect(onEdit).not.toHaveBeenCalled();

    // Unlock restores editability
    act(() => {
      uiStore.setState({ isDocumentLocked: false });
    });
    expect(
      (screen.getByRole('button', { name: /editor\.edit_label/i }) as HTMLButtonElement).disabled
    ).toBe(false);
  });
});
