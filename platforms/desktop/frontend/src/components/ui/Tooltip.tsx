import type React from 'react';
import {
  cloneElement,
  isValidElement,
  useCallback,
  useEffect,
  useId,
  useLayoutEffect,
  useRef,
  useState,
} from 'react';
import { ModalPortal } from '../ModalPortal';

export type TooltipPlacement = 'top' | 'bottom' | 'left' | 'right' | 'bottom-left' | 'bottom-right';

export interface TooltipProps {
  content?: React.ReactNode | false;
  placement?: TooltipPlacement;
  multiline?: boolean;
  delayMs?: number;
  disabled?: boolean;
  triggerDisabled?: boolean;
  onlyWhenTruncated?: boolean;
  fullWidthWrapper?: boolean;
  children: React.ReactElement<Record<string, unknown>>;
}

const DEFAULT_DELAY_MS = 250;
const VIEWPORT_MARGIN = 8;
const TRIGGER_GAP = 6;

const useIsomorphicLayoutEffect = typeof window !== 'undefined' ? useLayoutEffect : useEffect;

let activeTooltipCloser: (() => void) | null = null;

function claimActiveTooltip(closeSelf: () => void): void {
  if (activeTooltipCloser && activeTooltipCloser !== closeSelf) {
    activeTooltipCloser();
  }
  activeTooltipCloser = closeSelf;
}

function releaseActiveTooltip(closeSelf: () => void): void {
  if (activeTooltipCloser === closeSelf) {
    activeTooltipCloser = null;
  }
}

export function isTooltipContentVisible(content: React.ReactNode | false | undefined): boolean {
  return (
    content !== false &&
    content !== null &&
    content !== undefined &&
    (typeof content !== 'string' || content.trim().length > 0)
  );
}

function assignRef<T>(ref: React.Ref<T> | undefined, value: T | null): void {
  if (!ref) return;
  if (typeof ref === 'function') {
    ref(value);
  } else {
    (ref as React.RefObject<T | null>).current = value;
  }
}

function isElementTruncated(el: HTMLElement | null): boolean {
  if (!el) return false;
  return el.scrollWidth > el.clientWidth + 1 || el.scrollHeight > el.clientHeight + 1;
}

function computeTooltipCoordinates(
  triggerRect: DOMRect,
  tooltipWidth: number,
  tooltipHeight: number,
  preferredPlacement: TooltipPlacement
): { top: number; left: number; resolvedPlacement: TooltipPlacement } {
  const viewportWidth = typeof window !== 'undefined' ? window.innerWidth : 1024;
  const viewportHeight = typeof window !== 'undefined' ? window.innerHeight : 768;

  const fitsTop = triggerRect.top - tooltipHeight - TRIGGER_GAP >= VIEWPORT_MARGIN;
  const fitsBottom =
    triggerRect.bottom + tooltipHeight + TRIGGER_GAP <= viewportHeight - VIEWPORT_MARGIN;
  const fitsLeft = triggerRect.left - tooltipWidth - TRIGGER_GAP >= VIEWPORT_MARGIN;
  const fitsRight =
    triggerRect.right + tooltipWidth + TRIGGER_GAP <= viewportWidth - VIEWPORT_MARGIN;

  const hasTriggerGeometry =
    triggerRect.width > 0 ||
    triggerRect.height > 0 ||
    triggerRect.top !== 0 ||
    triggerRect.left !== 0;

  let resolvedPlacement = preferredPlacement;
  if (hasTriggerGeometry && preferredPlacement === 'top' && !fitsTop && fitsBottom) {
    resolvedPlacement = 'bottom';
  } else if (
    hasTriggerGeometry &&
    (preferredPlacement === 'bottom' ||
      preferredPlacement === 'bottom-left' ||
      preferredPlacement === 'bottom-right') &&
    !fitsBottom &&
    fitsTop
  ) {
    resolvedPlacement = 'top';
  } else if (hasTriggerGeometry && preferredPlacement === 'left' && !fitsLeft && fitsRight) {
    resolvedPlacement = 'right';
  } else if (hasTriggerGeometry && preferredPlacement === 'right' && !fitsRight && fitsLeft) {
    resolvedPlacement = 'left';
  }

  let top = 0;
  let left = 0;

  switch (resolvedPlacement) {
    case 'bottom':
      top = triggerRect.bottom + TRIGGER_GAP;
      left = triggerRect.left + triggerRect.width / 2 - tooltipWidth / 2;
      break;
    case 'bottom-left':
      top = triggerRect.bottom + TRIGGER_GAP;
      left = triggerRect.right - tooltipWidth;
      break;
    case 'bottom-right':
      top = triggerRect.bottom + TRIGGER_GAP;
      left = triggerRect.left;
      break;
    case 'left':
      top = triggerRect.top + triggerRect.height / 2 - tooltipHeight / 2;
      left = triggerRect.left - tooltipWidth - TRIGGER_GAP;
      break;
    case 'right':
      top = triggerRect.top + triggerRect.height / 2 - tooltipHeight / 2;
      left = triggerRect.right + TRIGGER_GAP;
      break;
    default:
      top = triggerRect.top - tooltipHeight - TRIGGER_GAP;
      left = triggerRect.left + triggerRect.width / 2 - tooltipWidth / 2;
      break;
  }

  if (viewportWidth > VIEWPORT_MARGIN * 2) {
    const maxLeft = Math.max(VIEWPORT_MARGIN, viewportWidth - tooltipWidth - VIEWPORT_MARGIN);
    left = Math.max(VIEWPORT_MARGIN, Math.min(left, maxLeft));
  }
  if (viewportHeight > VIEWPORT_MARGIN * 2) {
    const maxTop = Math.max(VIEWPORT_MARGIN, viewportHeight - tooltipHeight - VIEWPORT_MARGIN);
    top = Math.max(VIEWPORT_MARGIN, Math.min(top, maxTop));
  }

  return { top, left, resolvedPlacement };
}

export interface UseTooltipControllerOptions {
  placement?: TooltipPlacement;
  multiline?: boolean;
  delayMs?: number;
  disabled?: boolean;
  onlyWhenTruncated?: boolean;
}

export interface TooltipController {
  isOpen: boolean;
  tooltipId: string;
  triggerRef: React.RefObject<HTMLElement | null>;
  pointerDownRef: React.MutableRefObject<boolean>;
  openWithDelay: (triggerEl?: HTMLElement | null) => void;
  openImmediatelyOnFocus: (triggerEl?: HTMLElement | null) => void;
  hideTooltip: () => void;
  reposition: () => void;
  renderPortal: (content: React.ReactNode) => React.JSX.Element | null;
}

/**
 * Shared portal tooltip controller for both single-trigger `<Tooltip>` wrappers and
 * container-level delegated tooltips (e.g. dense token lists).
 */
export function useTooltipController({
  placement = 'top',
  multiline = false,
  delayMs = DEFAULT_DELAY_MS,
  disabled = false,
  onlyWhenTruncated = false,
}: UseTooltipControllerOptions = {}): TooltipController {
  const [isOpen, setIsOpen] = useState(false);
  const [coords, setCoords] = useState<{
    top: number;
    left: number;
    resolvedPlacement: TooltipPlacement;
  }>({
    top: 0,
    left: 0,
    resolvedPlacement: placement,
  });

  const triggerRef = useRef<HTMLElement | null>(null);
  const tooltipRef = useRef<HTMLDivElement | null>(null);
  const timerRef = useRef<number | null>(null);
  const pointerDownRef = useRef(false);
  const tooltipId = useId();

  const clearHoverTimer = useCallback(() => {
    if (timerRef.current !== null) {
      window.clearTimeout(timerRef.current);
      timerRef.current = null;
    }
  }, []);

  const hideTooltip = useCallback(() => {
    pointerDownRef.current = false;
    clearHoverTimer();
    releaseActiveTooltip(hideTooltip);
    setIsOpen(false);
  }, [clearHoverTimer]);

  const showNow = useCallback(
    (explicitTrigger?: HTMLElement | null) => {
      if (explicitTrigger !== undefined) {
        triggerRef.current = explicitTrigger;
      }
      if (disabled) return;
      if (onlyWhenTruncated && !isElementTruncated(triggerRef.current)) {
        return;
      }
      claimActiveTooltip(hideTooltip);
      setIsOpen(true);
    },
    [disabled, hideTooltip, onlyWhenTruncated]
  );

  const openWithDelay = useCallback(
    (explicitTrigger?: HTMLElement | null) => {
      if (explicitTrigger !== undefined) {
        triggerRef.current = explicitTrigger;
      }
      if (disabled) return;
      if (onlyWhenTruncated && !isElementTruncated(triggerRef.current)) {
        return;
      }
      clearHoverTimer();
      if (delayMs <= 0) {
        showNow();
        return;
      }
      timerRef.current = window.setTimeout(() => {
        timerRef.current = null;
        showNow();
      }, delayMs);
    },
    [clearHoverTimer, delayMs, disabled, onlyWhenTruncated, showNow]
  );

  const openImmediatelyOnFocus = useCallback(
    (explicitTrigger?: HTMLElement | null) => {
      if (pointerDownRef.current) {
        pointerDownRef.current = false;
        return;
      }
      clearHoverTimer();
      showNow(explicitTrigger);
    },
    [clearHoverTimer, showNow]
  );

  const reposition = useCallback(() => {
    const triggerEl = triggerRef.current;
    const tooltipEl = tooltipRef.current;
    if (!triggerEl || !tooltipEl) return;

    const triggerRect = triggerEl.getBoundingClientRect();
    const tooltipRect = tooltipEl.getBoundingClientRect();
    const next = computeTooltipCoordinates(
      triggerRect,
      tooltipRect.width || tooltipEl.offsetWidth || 0,
      tooltipRect.height || tooltipEl.offsetHeight || 0,
      placement
    );
    setCoords((prev) =>
      prev.top === next.top &&
      prev.left === next.left &&
      prev.resolvedPlacement === next.resolvedPlacement
        ? prev
        : next
    );
  }, [placement]);

  useIsomorphicLayoutEffect(() => {
    if (!isOpen || disabled) return;
    reposition();
  }, [isOpen, disabled, multiline, reposition]);

  useIsomorphicLayoutEffect(() => {
    if (!isOpen || disabled) return undefined;

    const handleScrollOrResize = () => hideTooltip();
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        hideTooltip();
      }
    };

    window.addEventListener('resize', handleScrollOrResize);
    window.addEventListener('scroll', handleScrollOrResize, true);
    window.addEventListener('keydown', handleKeyDown);
    return () => {
      window.removeEventListener('resize', handleScrollOrResize);
      window.removeEventListener('scroll', handleScrollOrResize, true);
      window.removeEventListener('keydown', handleKeyDown);
    };
  }, [hideTooltip, isOpen, disabled]);

  useEffect(() => {
    if (disabled && isOpen) {
      hideTooltip();
    }
  }, [disabled, hideTooltip, isOpen]);

  useEffect(() => {
    return () => {
      clearHoverTimer();
      releaseActiveTooltip(hideTooltip);
    };
  }, [clearHoverTimer, hideTooltip]);

  const renderPortal = (content: React.ReactNode): React.JSX.Element | null => {
    if (!isOpen || disabled || !isTooltipContentVisible(content)) {
      return null;
    }
    return (
      <ModalPortal>
        <div
          ref={tooltipRef}
          id={tooltipId}
          role="tooltip"
          data-placement={coords.resolvedPlacement}
          className={`tooltip-portal is-visible ${multiline ? 'tooltip-portal--multiline' : ''}`.trim()}
          style={{
            top: `${coords.top}px`,
            left: `${coords.left}px`,
          }}
        >
          {content}
        </div>
      </ModalPortal>
    );
  };

  return {
    isOpen,
    tooltipId,
    triggerRef,
    pointerDownRef,
    openWithDelay,
    openImmediatelyOnFocus,
    hideTooltip,
    reposition,
    renderPortal,
  };
}

export function Tooltip({
  content,
  placement = 'top',
  multiline = false,
  delayMs = DEFAULT_DELAY_MS,
  disabled = false,
  triggerDisabled = false,
  onlyWhenTruncated = false,
  fullWidthWrapper = false,
  children,
}: TooltipProps): React.JSX.Element {
  const hasContent = !disabled && isTooltipContentVisible(content);
  const {
    isOpen,
    tooltipId,
    triggerRef,
    pointerDownRef,
    openWithDelay,
    openImmediatelyOnFocus,
    hideTooltip,
    reposition,
    renderPortal,
  } = useTooltipController({
    placement,
    multiline,
    delayMs,
    disabled: !hasContent,
    onlyWhenTruncated,
  });

  useIsomorphicLayoutEffect(() => {
    if (!isOpen || !hasContent) return;
    // Reposition whenever rendered tooltip text/wrapping changes while open.
    void content;
    void multiline;
    reposition();
  }, [isOpen, hasContent, content, multiline, reposition]);

  if (!isValidElement(children)) {
    return children;
  }

  if (!hasContent) {
    return children;
  }

  const portalElement = renderPortal(content);

  if (triggerDisabled) {
    return (
      <>
        <span
          ref={(node) => {
            triggerRef.current = node;
          }}
          className={`tooltip-disabled-wrap ${fullWidthWrapper ? 'tooltip-disabled-wrap--full' : ''}`.trim()}
          tabIndex={0}
          aria-describedby={isOpen ? tooltipId : undefined}
          onMouseDown={() => {
            pointerDownRef.current = true;
          }}
          onMouseUp={() => {
            pointerDownRef.current = false;
          }}
          onMouseEnter={() => openWithDelay()}
          onMouseLeave={hideTooltip}
          onFocus={() => openImmediatelyOnFocus()}
          onBlur={hideTooltip}
        >
          {children}
        </span>
        {portalElement}
      </>
    );
  }

  const childProps = children.props as Record<string, unknown>;
  const existingDescribedBy =
    typeof childProps['aria-describedby'] === 'string' ? childProps['aria-describedby'] : undefined;
  const mergedDescribedBy = isOpen
    ? existingDescribedBy
      ? `${existingDescribedBy} ${tooltipId}`
      : tooltipId
    : existingDescribedBy;

  const childRef = childProps.ref as React.Ref<HTMLElement> | undefined;

  return (
    <>
      {cloneElement(children, {
        ref: (node: HTMLElement | null) => {
          triggerRef.current = node;
          assignRef(childRef, node);
        },
        'aria-describedby': mergedDescribedBy,
        onMouseDown: (event: React.MouseEvent<HTMLElement>) => {
          pointerDownRef.current = true;
          const handler = childProps.onMouseDown as
            | ((e: React.MouseEvent<HTMLElement>) => void)
            | undefined;
          handler?.(event);
        },
        onMouseUp: (event: React.MouseEvent<HTMLElement>) => {
          pointerDownRef.current = false;
          const handler = childProps.onMouseUp as
            | ((e: React.MouseEvent<HTMLElement>) => void)
            | undefined;
          handler?.(event);
        },
        onMouseEnter: (event: React.MouseEvent<HTMLElement>) => {
          openWithDelay();
          const handler = childProps.onMouseEnter as
            | ((e: React.MouseEvent<HTMLElement>) => void)
            | undefined;
          handler?.(event);
        },
        onMouseLeave: (event: React.MouseEvent<HTMLElement>) => {
          hideTooltip();
          const handler = childProps.onMouseLeave as
            | ((e: React.MouseEvent<HTMLElement>) => void)
            | undefined;
          handler?.(event);
        },
        onFocus: (event: React.FocusEvent<HTMLElement>) => {
          openImmediatelyOnFocus();
          const handler = childProps.onFocus as
            | ((e: React.FocusEvent<HTMLElement>) => void)
            | undefined;
          handler?.(event);
        },
        onBlur: (event: React.FocusEvent<HTMLElement>) => {
          hideTooltip();
          const handler = childProps.onBlur as
            | ((e: React.FocusEvent<HTMLElement>) => void)
            | undefined;
          handler?.(event);
        },
      })}
      {portalElement}
    </>
  );
}
