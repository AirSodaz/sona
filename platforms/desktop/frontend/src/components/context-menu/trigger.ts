import type React from 'react';

export interface ContextMenuOpenRequest {
  anchor: HTMLElement;
  point: { x: number; y: number };
  invocation: 'pointer' | 'keyboard';
}

export function createPointerContextMenuRequest(
  event: React.MouseEvent<HTMLElement>,
): ContextMenuOpenRequest {
  return {
    anchor: event.currentTarget,
    point: { x: event.clientX, y: event.clientY },
    invocation: 'pointer',
  };
}

export function createKeyboardContextMenuRequest(
  anchor: HTMLElement,
): ContextMenuOpenRequest {
  const rect = anchor.getBoundingClientRect();
  return {
    anchor,
    point: { x: rect.left + 12, y: rect.top + 12 },
    invocation: 'keyboard',
  };
}

export function isContextMenuKeyboardEvent(
  event: React.KeyboardEvent<HTMLElement>,
): boolean {
  return event.key === 'ContextMenu' || (event.key === 'F10' && event.shiftKey);
}
