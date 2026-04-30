import { create } from 'zustand';
import type { AppMode, ProcessingStatus } from '../types/transcript';

export interface TranscriptRuntimeState {
  mode: AppMode;
  processingStatus: ProcessingStatus;
  processingProgress: number;
  isRecording: boolean;
  isCaptionMode: boolean;
  isPaused: boolean;
  setMode: (mode: AppMode) => void;
  setProcessingStatus: (status: ProcessingStatus) => void;
  setProcessingProgress: (progress: number) => void;
  setIsRecording: (isRecording: boolean) => void;
  setIsCaptionMode: (isCaptionMode: boolean) => void;
  setIsPaused: (isPaused: boolean) => void;
}

export const useTranscriptRuntimeStore = create<TranscriptRuntimeState>((set) => ({
  mode: 'live',
  processingStatus: 'idle',
  processingProgress: 0,
  isRecording: false,
  isCaptionMode: false,
  isPaused: false,

  setMode: (mode) => set({ mode }),
  setProcessingStatus: (processingStatus) => set({ processingStatus }),
  setProcessingProgress: (processingProgress) => set({ processingProgress }),
  setIsRecording: (isRecording) => set({ isRecording }),
  setIsCaptionMode: (isCaptionMode) => set({ isCaptionMode }),
  setIsPaused: (isPaused) => set({ isPaused }),
}));
