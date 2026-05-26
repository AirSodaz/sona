import React, { useEffect, useRef } from 'react';
import { getFocusableElements, isTopMostModal } from '../utils/focusUtils';
import { useEscapeKey } from './useEscapeKey';

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

    useEscapeKey((e) => {
        e.preventDefault();
        onCloseRef.current();
    }, {
        enabled: isOpen,
        checkTopMost: true,
        containerRef
    });

    useEffect(() => {
        if (isOpen) {
            const previousFocus = document.activeElement as HTMLElement;
            // Wait for render
            requestAnimationFrame(() => {
                containerRef.current?.focus();
            });

            function handleKeyDown(e: KeyboardEvent) {
                const isTopMost = isTopMostModal(containerRef.current);

                if (e.key === 'Tab' && !e.ctrlKey) {
                    if (!isTopMost || !containerRef.current) return;

                    // Trap focus inside modal
                    const focusableElements = getFocusableElements(containerRef.current);

                    if (focusableElements.length === 0) return;

                    const firstElement = focusableElements[0];
                    const lastElement = focusableElements[focusableElements.length - 1];

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
