import type { ReactNode } from 'react';
import { createPortal } from 'react-dom';

interface ModalPortalProps {
    children: ReactNode;
}

export function ModalPortal({ children }: ModalPortalProps): React.JSX.Element {
    if (typeof document === 'undefined') {
        return <>{children}</>;
    }

    return createPortal(children, document.body);
}
