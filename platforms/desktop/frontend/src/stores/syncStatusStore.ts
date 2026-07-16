import { create } from 'zustand';
import {
  DISABLED_SYNC_STATUS,
  type SyncRunResult,
  type SyncStatusSnapshot,
} from '../types/sync';

interface SyncStatusState {
  snapshot: SyncStatusSnapshot;
  isLoaded: boolean;
  lastRunResult: SyncRunResult | null;
  setSnapshot: (snapshot: SyncStatusSnapshot) => void;
  setLastRunResult: (result: SyncRunResult) => void;
}

export const useSyncStatusStore = create<SyncStatusState>((set) => ({
  snapshot: DISABLED_SYNC_STATUS,
  isLoaded: false,
  lastRunResult: null,
  setSnapshot: (snapshot) => set({ snapshot, isLoaded: true }),
  setLastRunResult: (lastRunResult) => set({ lastRunResult }),
}));
