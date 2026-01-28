import { describe, it, expect, beforeEach } from 'vitest';
import { useDialogStore } from '../dialogStore';

describe('dialogStore', () => {
    beforeEach(() => {
        useDialogStore.setState({
            isOpen: false,
            options: null,
            resolveRef: null
        });
    });

    it('should open alert dialog and resolve when closed', async () => {
        const { alert, close } = useDialogStore.getState();

        let resolved = false;
        const promise = alert('Hello').then(() => {
            resolved = true;
        });

        expect(useDialogStore.getState().isOpen).toBe(true);
        expect(useDialogStore.getState().options?.message).toBe('Hello');
        expect(useDialogStore.getState().options?.type).toBe('alert');

        expect(resolved).toBe(false);

        // Close it
        close(true);

        await promise;
        expect(resolved).toBe(true);
        expect(useDialogStore.getState().isOpen).toBe(false);
    });

    it('should open confirm dialog and resolve with true when confirmed', async () => {
        const { confirm, close } = useDialogStore.getState();

        const promise = confirm('Are you sure?');

        expect(useDialogStore.getState().isOpen).toBe(true);
        expect(useDialogStore.getState().options?.type).toBe('confirm');

        close(true);

        const result = await promise;
        expect(result).toBe(true);
    });

    it('should open confirm dialog and resolve with false when cancelled', async () => {
        const { confirm, close } = useDialogStore.getState();

        const promise = confirm('Are you sure?');

        expect(useDialogStore.getState().isOpen).toBe(true);

        close(false);

        const result = await promise;
        expect(result).toBe(false);
    });
});
