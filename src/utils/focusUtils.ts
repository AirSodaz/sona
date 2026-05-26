export const MODAL_OVERLAY_SELECTOR =
  '.shared-modal-overlay, .panel-modal-overlay, .settings-overlay, [data-focus-trap-overlay]';

export const FOCUSABLE_SELECTOR = [
  'button:not([disabled])',
  '[href]',
  'input:not([disabled])',
  'select:not([disabled])',
  'textarea:not([disabled])',
  '[tabindex]:not([tabindex="-1"])',
].join(', ');

function isDisabledForFocus(element: HTMLElement): boolean {
  return (
    element.getAttribute('aria-disabled') === 'true'
    || element.hasAttribute('disabled')
    || element.closest('[aria-hidden="true"], [hidden], [inert]') !== null
  );
}

export function getFocusableElements(container: HTMLElement): HTMLElement[] {
  return Array.from(container.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR))
    .filter((element) => !isDisabledForFocus(element));
}

export function isGlobalDialogOpen(): boolean {
  return document.querySelector('[data-modal-layer="global-dialog"]') !== null;
}

export function isTopMostModal(container: HTMLElement | null): boolean {
  if (!container) {
    return false;
  }

  if (isGlobalDialogOpen() && !container.closest('[data-modal-layer="global-dialog"]')) {
    return false;
  }

  const overlays = document.querySelectorAll<HTMLElement>(MODAL_OVERLAY_SELECTOR);
  const topOverlay = overlays[overlays.length - 1];
  return !topOverlay || topOverlay.contains(container);
}
