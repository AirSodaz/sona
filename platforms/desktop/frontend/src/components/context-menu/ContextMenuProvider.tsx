import React, {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from 'react';
import { ContextMenuSurface } from './ContextMenuSurface';
import type {
  ContextMenuAction,
  ContextMenuCloseReason,
  OpenContextMenuOptions,
} from './types';
import { ContextMenuContext } from './useContextMenu';

export type {
  ContextMenuAction,
  ContextMenuCloseReason,
  OpenContextMenuOptions,
} from './types';

export function ContextMenuProvider({ children }: React.PropsWithChildren): React.JSX.Element {
  const [menu, setMenu] = useState<OpenContextMenuOptions | null>(null);
  const menuRef = useRef<OpenContextMenuOptions | null>(null);

  const openContextMenu = useCallback((options: OpenContextMenuOptions) => {
    const previousMenu = menuRef.current;
    menuRef.current = null;
    previousMenu?.onClose?.('replaced');
    menuRef.current = options;
    setMenu(options);
  }, []);

  const closeWithReason = useCallback((reason: ContextMenuCloseReason) => {
    const currentMenu = menuRef.current;
    if (!currentMenu) {
      return;
    }

    menuRef.current = null;
    setMenu(null);
    currentMenu.onClose?.(reason);
  }, []);

  const closeContextMenu = useCallback(() => {
    closeWithReason('programmatic');
  }, [closeWithReason]);

  const handleAction = useCallback((action: ContextMenuAction) => {
    closeWithReason('action');
    action.onSelect();
  }, [closeWithReason]);

  const handleDismiss = useCallback((reason: ContextMenuCloseReason) => {
    const anchor = menuRef.current?.anchor ?? null;
    closeWithReason(reason);

    if (reason === 'escape' && anchor?.isConnected) {
      anchor.focus();
    }
  }, [closeWithReason]);

  useEffect(() => {
    const handlePointerDown = (event: PointerEvent) => {
      const surface = document.querySelector<HTMLElement>('[data-context-menu-surface="true"]');
      if (surface?.contains(event.target as Node)) {
        return;
      }
      closeWithReason('outside');
    };
    const handleContextMenu = () => closeWithReason('outside');
    const handleScroll = () => closeWithReason('scroll');
    const handleResize = () => closeWithReason('resize');
    const handleBlur = () => closeWithReason('blur');

    window.addEventListener('pointerdown', handlePointerDown, true);
    window.addEventListener('contextmenu', handleContextMenu, true);
    window.addEventListener('scroll', handleScroll, true);
    window.addEventListener('resize', handleResize);
    window.addEventListener('blur', handleBlur);

    return () => {
      window.removeEventListener('pointerdown', handlePointerDown, true);
      window.removeEventListener('contextmenu', handleContextMenu, true);
      window.removeEventListener('scroll', handleScroll, true);
      window.removeEventListener('resize', handleResize);
      window.removeEventListener('blur', handleBlur);

      const currentMenu = menuRef.current;
      menuRef.current = null;
      currentMenu?.onClose?.('programmatic');
    };
  }, [closeWithReason]);

  const activeContextId = menu?.contextId ?? null;

  const value = useMemo(() => ({
    activeContextId,
    openContextMenu,
    closeContextMenu,
  }), [activeContextId, closeContextMenu, openContextMenu]);

  return (
    <ContextMenuContext.Provider value={value}>
      {children}
      {menu && (
        <ContextMenuSurface
          actions={menu.actions}
          ariaLabel={menu.ariaLabel}
          onAction={handleAction}
          onDismiss={handleDismiss}
          point={menu.point}
        />
      )}
    </ContextMenuContext.Provider>
  );
}
