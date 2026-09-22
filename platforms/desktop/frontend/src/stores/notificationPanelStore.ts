import { create } from 'zustand';

interface NotificationPanelState {
  isOpen: boolean;
  /** Entry id to scroll to after opening (e.g. from a toast). */
  focusEntryId: string | null;
  toggle: () => void;
  open: (focusEntryId?: string) => void;
  close: () => void;
}

export const useNotificationPanelStore = create<NotificationPanelState>((set) => ({
  isOpen: false,
  focusEntryId: null,
  toggle: () => set((state) => ({ isOpen: !state.isOpen, focusEntryId: null })),
  open: (focusEntryId?: string) => set({ isOpen: true, focusEntryId: focusEntryId ?? null }),
  close: () => set({ isOpen: false, focusEntryId: null }),
}));
