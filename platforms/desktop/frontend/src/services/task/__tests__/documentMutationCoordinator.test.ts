import { beforeEach, describe, expect, it, vi } from 'vitest';
import {
  DocumentMutationCoordinator,
  selectIsDocumentEditorLocked,
  useDocumentMutationLockStore,
} from '../documentMutationCoordinator';

describe('DocumentMutationCoordinator', () => {
  let coordinator: DocumentMutationCoordinator;

  beforeEach(() => {
    useDocumentMutationLockStore.setState({ locks: {} });
    coordinator = new DocumentMutationCoordinator();
  });

  it('serializes concurrent mutations for the same document FIFO after flushing active editor and auto-save', async () => {
    const events: string[] = [];
    const gateA = Promise.withResolvers<void>();

    const first = coordinator.runExclusive(
      { sessionId: 'sess-1', historyId: 'hist-1' },
      {
        taskType: 'polish',
        flushActiveEditor: () => events.push('flushEditor:A'),
        flushAutoSave: async (ids: string[]) => {
          events.push(`flushAutoSave:A:${ids.join(',')}`);
        },
      },
      async (ctx) => {
        events.push(`start:A:${ctx.docKey}`);
        expect(
          selectIsDocumentEditorLocked(
            useDocumentMutationLockStore.getState().locks,
            'sess-1',
            'hist-1'
          )
        ).toBe(true);
        await gateA.promise;
        events.push('end:A');
      }
    );

    const second = coordinator.runExclusive(
      { sessionId: 'sess-1', historyId: 'hist-1' },
      {
        taskType: 'translate',
        flushActiveEditor: () => events.push('flushEditor:B'),
        flushAutoSave: async (ids: string[]) => {
          events.push(`flushAutoSave:B:${ids.join(',')}`);
        },
      },
      async () => {
        events.push('start:B');
        events.push('end:B');
      }
    );

    await vi.waitFor(() => {
      expect(events).toEqual(['flushEditor:A', 'flushAutoSave:A:hist-1', 'start:A:hist-1']);
    });

    gateA.resolve();
    await Promise.all([first, second]);

    expect(events).toEqual([
      'flushEditor:A',
      'flushAutoSave:A:hist-1',
      'start:A:hist-1',
      'end:A',
      'flushEditor:B',
      'flushAutoSave:B:hist-1',
      'start:B',
      'end:B',
    ]);
    expect(
      selectIsDocumentEditorLocked(
        useDocumentMutationLockStore.getState().locks,
        'sess-1',
        'hist-1'
      )
    ).toBe(false);
  });

  it('allows mutations on different documents to execute concurrently', async () => {
    const gateA = Promise.withResolvers<void>();
    let startedBWhileAHeld = false;

    const first = coordinator.runExclusive(
      { sessionId: 'sess-a', historyId: 'hist-a' },
      { taskType: 'polish' },
      async () => {
        await gateA.promise;
      }
    );

    const second = coordinator.runExclusive(
      { sessionId: 'sess-b', historyId: 'hist-b' },
      { taskType: 'translate' },
      async () => {
        startedBWhileAHeld = true;
      }
    );

    await second;
    expect(startedBWhileAHeld).toBe(true);

    gateA.resolve();
    await first;
  });

  it('releases editor lock even when mutation fails and keeps editor unlocked when lockEditor=false', async () => {
    const flushActiveEditor = vi.fn();

    await expect(
      coordinator.runExclusive(
        { sessionId: 'sess-err', historyId: 'hist-err' },
        { taskType: 'retranscribe', flushActiveEditor },
        async () => {
          expect(
            selectIsDocumentEditorLocked(
              useDocumentMutationLockStore.getState().locks,
              'sess-err',
              'hist-err'
            )
          ).toBe(true);
          throw new Error('mutation failed');
        }
      )
    ).rejects.toThrow('mutation failed');

    expect(flushActiveEditor).toHaveBeenCalledTimes(1);
    expect(
      selectIsDocumentEditorLocked(
        useDocumentMutationLockStore.getState().locks,
        'sess-err',
        'hist-err'
      )
    ).toBe(false);

    flushActiveEditor.mockClear();
    await coordinator.runExclusive(
      { sessionId: 'sess-bg', historyId: 'hist-bg' },
      { taskType: 'speaker_annotation', lockEditor: false, flushActiveEditor },
      async () => {
        expect(
          selectIsDocumentEditorLocked(
            useDocumentMutationLockStore.getState().locks,
            'sess-bg',
            'hist-bg'
          )
        ).toBe(false);
      }
    );
    expect(flushActiveEditor).not.toHaveBeenCalled();
  });

  it('re-resolves latest historyId after waiting in FIFO queue for late-persisted sessions', async () => {
    const gate = Promise.withResolvers<void>();
    let savedHistoryId: string | null = null;
    let resolvedInSecond: string | null = null;

    const first = coordinator.runExclusive(
      { sessionId: 'draft-1', historyId: null },
      { taskType: 'polish' },
      async () => {
        savedHistoryId = 'hist-persisted';
        await gate.promise;
      }
    );

    const second = coordinator.runExclusive(
      { sessionId: 'draft-1', historyId: null },
      {
        taskType: 'translate',
        resolveLatestHistoryId: () => savedHistoryId,
      },
      async (ctx) => {
        resolvedInSecond = ctx.historyId;
      }
    );

    gate.resolve();
    await Promise.all([first, second]);

    expect(resolvedInSecond).toBe('hist-persisted');
  });
});
