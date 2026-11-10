import type React from 'react';
import { createContext, useContext, useMemo } from 'react';
import { useTranscriptStore } from '../stores/transcriptStore';

export interface TranscriptSessionContextValue {
  sessionId: string;
  isActive: boolean;
}

const TranscriptSessionContext = createContext<TranscriptSessionContextValue | null>(null);

export interface TranscriptSessionProviderProps {
  sessionId: string;
  isActive: boolean;
  children: React.ReactNode;
}

/**
 * Provides an explicit session boundary for host views (Create workbench vs Projects detail),
 * decoupling hidden or inactive editors from global `activeSessionId` changes.
 */
export function TranscriptSessionProvider({
  sessionId,
  isActive,
  children,
}: TranscriptSessionProviderProps): React.JSX.Element {
  const value = useMemo<TranscriptSessionContextValue>(
    () => ({ sessionId, isActive }),
    [sessionId, isActive]
  );
  return (
    <TranscriptSessionContext.Provider value={value}>{children}</TranscriptSessionContext.Provider>
  );
}

/**
 * Reads the enclosing `TranscriptSessionContext` if mounted inside a host provider.
 */
export function useOptionalTranscriptSessionContext(): TranscriptSessionContextValue | null {
  return useContext(TranscriptSessionContext);
}

/**
 * Resolves the target transcript `sessionId` from the nearest `TranscriptSessionProvider`,
 * falling back to global `activeSessionId` only for isolated tests mounted without a provider.
 */
export function useBoundSessionId(): string {
  const context = useContext(TranscriptSessionContext);
  const activeSessionId = useTranscriptStore((state) => state.activeSessionId);
  return context ? context.sessionId : activeSessionId;
}

/**
 * Returns whether the enclosing host session is currently active (visible/focused).
 * Defaults to `true` when mounted outside a provider.
 */
export function useBoundSessionIsActive(): boolean {
  const context = useContext(TranscriptSessionContext);
  return context ? context.isActive : true;
}
