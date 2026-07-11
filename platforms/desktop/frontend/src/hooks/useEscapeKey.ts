import React, { useEffect, useRef } from 'react';
import { isTopMostModal } from '../utils/focusUtils';

export interface UseEscapeKeyOptions {
  enabled?: boolean;
  checkTopMost?: boolean;
  containerRef?: React.RefObject<HTMLElement | null>;
}

export function useEscapeKey(
  onEscape: (e: KeyboardEvent) => void,
  options: UseEscapeKeyOptions = {}
) {
  const { enabled = true, checkTopMost = false, containerRef } = options;
  const callbackRef = useRef(onEscape);

  useEffect(() => {
    callbackRef.current = onEscape;
  }, [onEscape]);

  useEffect(() => {
    if (!enabled) return;

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        if (checkTopMost) {
          const isTopMost = isTopMostModal(containerRef?.current || null);
          if (!isTopMost) return;
        }
        callbackRef.current(e);
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [enabled, checkTopMost, containerRef]);
}
