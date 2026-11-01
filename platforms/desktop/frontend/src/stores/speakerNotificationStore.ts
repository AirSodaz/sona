import { create } from 'zustand';

export interface SpeakerNotificationState {
  hasDegradedNotice: boolean;
  degradedCount: number;
  triggerDegraded: () => void;
  dismissNotice: () => void;
}

export const useSpeakerNotificationStore = create<SpeakerNotificationState>((set) => ({
  hasDegradedNotice: false,
  degradedCount: 0,
  triggerDegraded: () =>
    set((state) => ({ hasDegradedNotice: true, degradedCount: state.degradedCount + 1 })),
  dismissNotice: () => set({ hasDegradedNotice: false, degradedCount: 0 }),
}));
