import type React from 'react';
import { useCallback, useRef } from 'react';

export interface TabNavigationItem<T extends string = string> {
  value: T;
  disabled?: boolean;
}

export interface UseTabKeyboardNavigationOptions<T extends string = string> {
  items: TabNavigationItem<T>[];
  activeTab: T;
  onChange: (value: T) => void;
  enabled?: boolean;
}

export function useTabKeyboardNavigation<T extends string = string>({
  items,
  activeTab,
  onChange,
  enabled = true,
}: UseTabKeyboardNavigationOptions<T>) {
  const buttonRefs = useRef<Map<T, HTMLButtonElement | null>>(new Map());

  const registerButtonRef = useCallback((value: T) => {
    return (el: HTMLButtonElement | null) => {
      if (el) {
        buttonRefs.current.set(value, el);
      } else {
        buttonRefs.current.delete(value);
      }
    };
  }, []);
  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (!enabled) return;

      const enabledItems = items.filter((item) => !item.disabled);
      if (enabledItems.length === 0) return;

      const currentIndex = enabledItems.findIndex((item) => item.value === activeTab);

      let targetItem: TabNavigationItem<T> | undefined;

      if (e.key === 'ArrowRight' || e.key === 'ArrowDown') {
        e.preventDefault();
        const nextIndex = currentIndex === -1 ? 0 : (currentIndex + 1) % enabledItems.length;
        targetItem = enabledItems[nextIndex];
      } else if (e.key === 'ArrowLeft' || e.key === 'ArrowUp') {
        e.preventDefault();
        const prevIndex =
          currentIndex === -1
            ? enabledItems.length - 1
            : (currentIndex - 1 + enabledItems.length) % enabledItems.length;
        targetItem = enabledItems[prevIndex];
      } else if (e.key === 'Home') {
        e.preventDefault();
        targetItem = enabledItems[0];
      } else if (e.key === 'End') {
        e.preventDefault();
        targetItem = enabledItems[enabledItems.length - 1];
      }

      if (targetItem) {
        onChange(targetItem.value);
        const button = buttonRefs.current.get(targetItem.value);
        if (button) {
          button.focus();
        }
      }
    },
    [enabled, items, activeTab, onChange]
  );
  return { registerButtonRef, handleKeyDown };
}
