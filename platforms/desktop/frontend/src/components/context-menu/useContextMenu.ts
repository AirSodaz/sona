import { createContext, useContext } from 'react';
import type { OpenContextMenuOptions } from './types';

export interface ContextMenuController {
  activeContextId: string | null;
  openContextMenu: (options: OpenContextMenuOptions) => void;
  closeContextMenu: () => void;
}

export const ContextMenuContext = createContext<ContextMenuController | null>(null);

export function useContextMenu(): ContextMenuController {
  const context = useContext(ContextMenuContext);
  if (!context) {
    throw new Error('useContextMenu must be used within a ContextMenuProvider');
  }
  return context;
}
