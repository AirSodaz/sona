import React, { useEffect, useRef } from 'react';


/**
 * Hook to trap focus inside a modal and handle Escape key.
 *
 * @param isOpen Whether the modal is open.
 * @param onClose Callback to close the modal.
 * @param containerRef Reference to the modal container element.
 */
export function useFocusTrap(
    isOpen: boolean,
    onClose: () => void,
    containerRef: React.RefObject<HTMLElement | null>
) {
    const onCloseRef = useRef(onClose);

    useEffect(() => {
        onCloseRef.current = onClose;
    }, [onClose]);

    useEffect(() => {
        if (isOpen) {
            const previousFocus = document.activeElement as HTMLElement;
            // Wait for render
            requestAnimationFrame(() => {
                containerRef.current?.focus();
            });

            function handleKeyDown(e: KeyboardEvent) {
                // If GlobalDialog is open, let it handle Escape
                if (document.querySelector('.dialog-modal')) return;

                const overlays = document.querySelectorAll('.shared-modal-overlay, .panel-modal-overlay, .settings-overlay, [data-focus-trap-overlay]');
                const topOverlay = overlays[overlays.length - 1];
                const isTopMost = !topOverlay || (containerRef.current && topOverlay.contains(containerRef.current));

                if (e.key === 'Escape') {
                    if (isTopMost) {
                        e.preventDefault();
                        onCloseRef.current();
                    }
                    return;
                }

                if (e.key === 'Tab' && !e.ctrlKey) {
                    if (!isTopMost || !containerRef.current) return;

                    // Trap focus inside modal
                    const focusableSelector = 'button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"]):not([disabled])';
                    const focusableElements = containerRef.current.querySelectorAll(focusableSelector);

                    if (focusableElements.length === 0) return;

                    const firstElement = focusableElements[0] as HTMLElement;
                    const lastElement = focusableElements[focusableElements.length - 1] as HTMLElement;

                    const isFocusInside = containerRef.current.contains(document.activeElement);

                    if (!isFocusInside) {
                        e.preventDefault();
                        firstElement.focus();
                    } else if (e.shiftKey) {
                        if (document.activeElement === firstElement) {
                            e.preventDefault();
                            lastElement.focus();
                        }
                    } else {
                        if (document.activeElement === lastElement) {
                            e.preventDefault();
                            firstElement.focus();
                        }
                    }
                }
            }
            window.addEventListener('keydown', handleKeyDown);

            return () => {
                window.removeEventListener('keydown', handleKeyDown);
                previousFocus?.focus();
            };
        }
    }, [isOpen, containerRef]);
}
