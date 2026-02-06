import { useEffect } from 'react';
import { useDialogStore } from '../stores/dialogStore';

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
    useEffect(() => {
        if (isOpen) {
            const previousFocus = document.activeElement as HTMLElement;
            // Wait for render
            requestAnimationFrame(() => {
                containerRef.current?.focus();
            });

            function handleKeyDown(e: KeyboardEvent) {
                if (e.key === 'Escape') {
                    // Only close if no other dialog is open (GlobalDialog)
                    if (useDialogStore.getState().isOpen) return;
                    onClose();
                    return;
                }

                if (e.key === 'Tab') {
                    if (!containerRef.current) return;

                    // Trap focus inside modal
                    const focusableSelector = 'button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"]):not([disabled])';
                    const focusableElements = containerRef.current.querySelectorAll(focusableSelector);

                    if (focusableElements.length === 0) return;

                    const firstElement = focusableElements[0] as HTMLElement;
                    const lastElement = focusableElements[focusableElements.length - 1] as HTMLElement;

                    if (e.shiftKey) {
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
    }, [isOpen, onClose, containerRef]);
}
