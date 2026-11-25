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

  if (viewportWidth > 0 && tooltipWidth > 0) {
    left = Math.max(
      VIEWPORT_MARGIN,
      Math.min(left, viewportWidth - tooltipWidth - VIEWPORT_MARGIN)
    );
  }
  if (viewportHeight > 0 && tooltipHeight > 0) {
    top = Math.max(
      VIEWPORT_MARGIN,
      Math.min(top, viewportHeight - tooltipHeight - VIEWPORT_MARGIN)
    );
  }

  return { top, left, resolvedPlacement };
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

  const hasContent =
    !disabled &&
    content !== false &&
    content !== null &&
    content !== undefined &&
    (typeof content !== 'string' || content.trim().length > 0);

  const clearHoverTimer = useCallback(() => {
    if (timerRef.current !== null) {
      window.clearTimeout(timerRef.current);
      timerRef.current = null;
    }
  }, []);

  const hideTooltip = useCallback(() => {
    clearHoverTimer();
    setIsOpen(false);
  }, [clearHoverTimer]);

  const canOpen = useCallback(() => {
    if (!hasContent) return false;
    if (onlyWhenTruncated && !isElementTruncated(triggerRef.current)) {
      return false;
    }
    return true;
  }, [hasContent, onlyWhenTruncated]);

  const openWithDelay = useCallback(() => {
    if (!canOpen()) return;
    clearHoverTimer();
    if (delayMs <= 0) {
      setIsOpen(true);
      return;
    }
    timerRef.current = window.setTimeout(() => {
      timerRef.current = null;
      setIsOpen(true);
    }, delayMs);
  }, [canOpen, clearHoverTimer, delayMs]);

  const openImmediatelyOnFocus = useCallback(() => {
    if (pointerDownRef.current) {
      pointerDownRef.current = false;
      return;
    }
    if (!canOpen()) return;
    clearHoverTimer();
    setIsOpen(true);
  }, [canOpen, clearHoverTimer]);

  const updatePosition = useCallback(() => {
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
    setCoords(next);
  }, [placement]);

  useIsomorphicLayoutEffect(() => {
    if (!isOpen || !hasContent) return undefined;
    updatePosition();

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
  }, [hideTooltip, isOpen, hasContent, updatePosition]);

  useEffect(() => {
    if (!hasContent && isOpen) {
      hideTooltip();
    }
  }, [hasContent, hideTooltip, isOpen]);

  useEffect(() => {
    return () => {
      clearHoverTimer();
    };
  }, [clearHoverTimer]);

  if (!isValidElement(children)) {
    return children;
  }

  if (!hasContent) {
    return (
      <>
        {children}
        {null}
      </>
    );
  }

  const portalElement = isOpen ? (
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
  ) : null;

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
          onMouseEnter={openWithDelay}
          onMouseLeave={hideTooltip}
          onFocus={openImmediatelyOnFocus}
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
