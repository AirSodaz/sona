export type AsyncStatus = 'idle' | 'loading' | 'success' | 'error';

export interface AsyncErrorState {
  error: string | null;
}

export interface LoadableState extends AsyncErrorState {
  isBusy: boolean;
  isLoaded: boolean;
}
