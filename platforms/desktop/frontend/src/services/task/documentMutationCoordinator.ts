import { create } from 'zustand';

export interface DocumentLockTarget {
  sessionId?: string | null;
  historyId?: string | null;
}

export interface DocumentMutationRunOptions<TContext = void> {
  taskType: string;
  /**
   * When true (default), locks inline segment editing & header rewrite actions on the target document
   * and commits any active editor before flushing auto-save.
   */
  lockEditor?: boolean;
  /** Commits/blurs any open inline segment editor bound to the target session. */
  flushActiveEditor?: () => void;
  /** Flushes any pending auto-save debounce for the target document key(s) before acquiring segments. */
  flushAutoSave?: (historyIds: string[]) => Promise<void>;
  /** Optional hook to resolve the latest historyId after waiting in the FIFO queue. */
  resolveLatestHistoryId?: () => string | null;
  context?: TContext;
}

export interface DocumentMutationLockContext {
  docKey: string;
  historyId: string | null;
}

interface LockEntry {
  depth: number;
  editorLockDepth: number;
  taskTypes: string[];
}

interface DocumentLockReactiveState {
  locks: Record<string, LockEntry>;
}

export const useDocumentMutationLockStore = create<DocumentLockReactiveState>(() => ({
  locks: {},
}));

export function toSessionDocumentKey(sessionId: string): string {
  return `session:${sessionId}`;
}

/**
 * Per-document FIFO async mutex coordinator (`Channel 3`).
 *
 * Ensures whole-document transcript mutations (Polish, Translate, Retranscribe, Pipeline segment steps,
 * VersionRestore, SpeakerCorrection, late persistence, and speaker-annotation merges) execute strictly
 * sequentially on the same document while allowing Summary generation on separate documents/sidecar
 * to proceed concurrently.
 */
export class DocumentMutationCoordinator {
  private tails = new Map<string, Promise<void>>();
  private sessionAliases = new Map<string, string>();

  private resolveRootKey(rawKey: string): string {
    let current = rawKey;
    const seen = new Set<string>();
    while (this.sessionAliases.has(current) && !seen.has(current)) {
      seen.add(current);
      const next = this.sessionAliases.get(current);
      if (!next) break;
      current = next;
    }
    return current;
  }

  resolveKeysForTarget(target: DocumentLockTarget): string[] {
    const keys = new Set<string>();
    const historyId = target.historyId && target.historyId !== 'current' ? target.historyId : null;
    if (historyId) {
      keys.add(historyId);
      keys.add(this.resolveRootKey(historyId));
    }
    if (target.sessionId) {
      const sessionKey = toSessionDocumentKey(target.sessionId);
      keys.add(sessionKey);
      keys.add(this.resolveRootKey(sessionKey));
    }
    if (keys.size === 0) {
      keys.add('session:default');
    }
    return Array.from(keys);
  }

  resolveCanonicalKey(target: DocumentLockTarget): string {
    if (target.historyId && target.historyId !== 'current') {
      return this.resolveRootKey(target.historyId);
    }
    if (target.sessionId) {
      return this.resolveRootKey(toSessionDocumentKey(target.sessionId));
    }
    return 'session:default';
  }

  /**
   * Aliases an unpersisted session key (`session:${sessionId}`) to its persisted `historyId`.
   * If a FIFO chain or active lock already exists on the session key, links the `historyId` queue
   * behind it so newly arriving history-keyed tasks wait for the in-flight session mutation.
   */
  rekeyAlias(sessionId: string, historyId: string): void {
    if (!sessionId || !historyId || historyId === 'current') {
      return;
    }
    const sessionKey = toSessionDocumentKey(sessionId);
    this.sessionAliases.set(sessionKey, historyId);

    const sessionTail = this.tails.get(sessionKey);
    if (sessionTail) {
      const prevHistoryTail = this.tails.get(historyId) ?? Promise.resolve();
      this.tails.set(
        historyId,
        Promise.allSettled([prevHistoryTail, sessionTail]).then(() => undefined)
      );
    }

    const existingSessionEntry = useDocumentMutationLockStore.getState().locks[sessionKey];
    if (existingSessionEntry) {
      useDocumentMutationLockStore.setState((state) => {
        const currentHistory = state.locks[historyId];
        return {
          locks: {
            ...state.locks,
            [historyId]: {
              depth: (currentHistory?.depth ?? 0) + existingSessionEntry.depth,
              editorLockDepth:
                (currentHistory?.editorLockDepth ?? 0) + existingSessionEntry.editorLockDepth,
              taskTypes: [...(currentHistory?.taskTypes ?? []), ...existingSessionEntry.taskTypes],
            },
          },
        };
      });
    }
  }

  async runExclusive<T>(
    target: DocumentLockTarget,
    options: DocumentMutationRunOptions,
    fn: (lockCtx: DocumentMutationLockContext) => Promise<T>
  ): Promise<T> {
    const initialKey = this.resolveCanonicalKey(target);
    const lockEditor = options.lockEditor !== false;

    const previousTail = this.tails.get(initialKey) ?? Promise.resolve();
    let releaseTail!: () => void;
    const myTail = new Promise<void>((resolve) => {
      releaseTail = resolve;
    });

    this.tails.set(
      initialKey,
      previousTail.then(
        () => myTail,
        () => myTail
      )
    );

    await previousTail.catch(() => undefined);

    const activeKeys = new Set<string>([initialKey, ...this.resolveKeysForTarget(target)]);
    const initialHistoryId =
      options.resolveLatestHistoryId?.() ??
      (target.historyId && target.historyId !== 'current' ? target.historyId : null);
    if (initialHistoryId) {
      activeKeys.add(initialHistoryId);
    }

    this.incrementLockEntries(activeKeys, options.taskType, lockEditor);

    try {
      if (lockEditor) {
        options.flushActiveEditor?.();
      }

      const resolvedHistoryId =
        options.resolveLatestHistoryId?.() ??
        (target.historyId && target.historyId !== 'current' ? target.historyId : null);

      const flushCandidateIds = Array.from(
        new Set(
          [
            target.historyId && target.historyId !== 'current' ? target.historyId : null,
            resolvedHistoryId,
          ].filter((id): id is string => Boolean(id))
        )
      );

      if (options.flushAutoSave && flushCandidateIds.length > 0) {
        await options.flushAutoSave(flushCandidateIds);
      }

      const finalHistoryId =
        options.resolveLatestHistoryId?.() ??
        resolvedHistoryId ??
        (target.historyId && target.historyId !== 'current' ? target.historyId : null);

      return await fn({
        docKey: finalHistoryId ?? initialKey,
        historyId: finalHistoryId,
      });
    } finally {
      const cleanupKeys = new Set<string>([...activeKeys, ...this.resolveKeysForTarget(target)]);
      this.decrementLockEntries(cleanupKeys, options.taskType, lockEditor);
      if (this.tails.get(initialKey) === myTail) {
        this.tails.delete(initialKey);
      }
      releaseTail();
    }
  }

  private incrementLockEntries(keys: Set<string>, taskType: string, lockEditor: boolean): void {
    useDocumentMutationLockStore.setState((state) => {
      const nextLocks = { ...state.locks };
      for (const key of keys) {
        const existing = nextLocks[key];
        nextLocks[key] = {
          depth: (existing?.depth ?? 0) + 1,
          editorLockDepth: (existing?.editorLockDepth ?? 0) + (lockEditor ? 1 : 0),
          taskTypes: [...(existing?.taskTypes ?? []), taskType],
        };
      }
      return { locks: nextLocks };
    });
  }

  private decrementLockEntries(keys: Set<string>, taskType: string, lockEditor: boolean): void {
    useDocumentMutationLockStore.setState((state) => {
      const nextLocks = { ...state.locks };
      for (const key of keys) {
        const existing = nextLocks[key];
        if (!existing) continue;
        const nextDepth = Math.max(0, existing.depth - 1);
        const nextEditorDepth = Math.max(0, existing.editorLockDepth - (lockEditor ? 1 : 0));
        if (nextDepth === 0) {
          delete nextLocks[key];
        } else {
          const nextTaskTypes = [...existing.taskTypes];
          const idx = nextTaskTypes.indexOf(taskType);
          if (idx >= 0) {
            nextTaskTypes.splice(idx, 1);
          }
          nextLocks[key] = {
            depth: nextDepth,
            editorLockDepth: nextEditorDepth,
            taskTypes: nextTaskTypes,
          };
        }
      }
      return { locks: nextLocks };
    });
  }

  isLocked(sessionId?: string | null, historyId?: string | null): boolean {
    const locks = useDocumentMutationLockStore.getState().locks;
    return this.resolveKeysForTarget({ sessionId, historyId }).some(
      (key) => (locks[key]?.depth ?? 0) > 0
    );
  }

  isEditorLocked(sessionId?: string | null, historyId?: string | null): boolean {
    const locks = useDocumentMutationLockStore.getState().locks;
    return this.resolveKeysForTarget({ sessionId, historyId }).some(
      (key) => (locks[key]?.editorLockDepth ?? 0) > 0
    );
  }

  hasActiveTasks(): boolean {
    const locks = useDocumentMutationLockStore.getState().locks;
    return Object.values(locks).some((entry) => entry.depth > 0);
  }

  resetForTests(): void {
    this.tails.clear();
    this.sessionAliases.clear();
    useDocumentMutationLockStore.setState({ locks: {} });
  }
}

export const documentMutationCoordinator = new DocumentMutationCoordinator();

export function selectIsDocumentEditorLocked(
  locks: Record<string, LockEntry>,
  sessionId?: string | null,
  historyId?: string | null
): boolean {
  const keys = documentMutationCoordinator.resolveKeysForTarget({ sessionId, historyId });
  return keys.some((key) => (locks[key]?.editorLockDepth ?? 0) > 0);
}
