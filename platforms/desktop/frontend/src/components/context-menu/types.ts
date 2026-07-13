import type React from 'react';

export type ContextMenuCloseReason =
  | 'action'
  | 'escape'
  | 'outside'
  | 'scroll'
  | 'resize'
  | 'blur'
  | 'replaced'
  | 'programmatic';

export interface ContextMenuAction {
  id: string;
  label: string;
  icon?: React.ReactNode;
  shortcut?: string;
  disabled?: boolean;
  tone?: 'default' | 'danger';
  dividerBefore?: boolean;
  onSelect: () => void;
}

export interface OpenContextMenuOptions {
  contextId: string;
  ariaLabel: string;
  actions: ContextMenuAction[];
  anchor: HTMLElement;
  point: { x: number; y: number };
  invocation: 'pointer' | 'keyboard';
  onClose?: (reason: ContextMenuCloseReason) => void;
}
