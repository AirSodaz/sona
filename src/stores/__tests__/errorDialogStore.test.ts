import { beforeEach, describe, expect, it } from 'vitest';
import { useErrorDialogStore } from '../errorDialogStore';

describe('errorDialogStore', () => {
  beforeEach(() => {
    useErrorDialogStore.setState({
      isOpen: false,
      options: null,
      resolveRef: null,
    });
  });

  it('opens a dismiss-only error dialog and resolves dismiss on close', async () => {
    const { showError, close } = useErrorDialogStore.getState();

    const promise = showError({
      title: 'Error',
      message: 'Something failed.',
      primaryLabel: 'OK',
      hasPrimaryAction: false,
    });

    expect(useErrorDialogStore.getState().isOpen).toBe(true);
    expect(useErrorDialogStore.getState().options?.message).toBe('Something failed.');

    close('dismiss');

    await expect(promise).resolves.toBe('dismiss');
    expect(useErrorDialogStore.getState().isOpen).toBe(false);
  });

  it('opens an actionable error dialog and resolves the primary action', async () => {
    const { showError, close } = useErrorDialogStore.getState();

    const promise = showError({
      title: 'Error',
      message: 'Update failed.',
      details: 'network timeout',
      primaryLabel: 'Download Manually',
      cancelLabel: 'Cancel',
      hasPrimaryAction: true,
    });

    expect(useErrorDialogStore.getState().options?.hasPrimaryAction).toBe(true);
    expect(useErrorDialogStore.getState().options?.cancelLabel).toBe('Cancel');

    close('primary');

    await expect(promise).resolves.toBe('primary');
    expect(useErrorDialogStore.getState().options).toBeNull();
  });
});
