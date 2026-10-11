// @vitest-environment jsdom
import { act, renderHook } from '@testing-library/react';
import { beforeEach, describe, expect, it } from 'vitest';
import { isAnyDialogOpen, useDialogStore, useIsAnyDialogOpen } from '../dialogStore';
import { useErrorDialogStore } from '../errorDialogStore';

describe('dialogStore', () => {
  beforeEach(() => {
    useDialogStore.setState({
      isOpen: false,
      options: null,
      resolveRef: null,
      checkboxResolveRef: null,
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

  it('should open a standardized error dialog with details', async () => {
    const { showError, close } = useDialogStore.getState();

    const promise = showError({
      code: 'translation.failed',
      messageKey: 'errors.translation.failed',
      cause: new Error('timeout'),
    });

    expect(useDialogStore.getState().isOpen).toBe(true);
    expect(useDialogStore.getState().options?.title).toBeTruthy();
    expect(useDialogStore.getState().options?.variant).toBe('error');
    expect(useDialogStore.getState().options?.details).toBe('timeout');

    close(true);

    await promise;
    expect(useDialogStore.getState().isOpen).toBe(false);
  });

  it('should open checkboxConfirm and resolve typed CheckboxConfirmResult when confirmed with checked=true', async () => {
    const { checkboxConfirm, closeCheckboxConfirm } = useDialogStore.getState();

    const promise = checkboxConfirm('Move to trash?', {
      checkbox: {
        label: 'Directly delete',
        checkedConfirmLabel: 'Delete Permanently',
      },
    });

    expect(useDialogStore.getState().isOpen).toBe(true);
    expect(useDialogStore.getState().options?.type).toBe('confirm');
    expect(useDialogStore.getState().options?.checkbox?.label).toBe('Directly delete');

    closeCheckboxConfirm({ confirmed: true, checked: true });

    const result = await promise;
    expect(result).toEqual({ confirmed: true, checked: true });
    expect(useDialogStore.getState().isOpen).toBe(false);
  });

  it('should open checkboxConfirm and resolve typed CheckboxConfirmResult when confirmed with checked=false', async () => {
    const { checkboxConfirm, closeCheckboxConfirm } = useDialogStore.getState();

    const promise = checkboxConfirm('Move to trash?', {
      checkbox: {
        label: 'Directly delete',
      },
    });

    closeCheckboxConfirm({ confirmed: true, checked: false });

    const result = await promise;
    expect(result).toEqual({ confirmed: true, checked: false });
  });

  it('should open checkboxConfirm and resolve with confirmed=false when cancelled via generic close', async () => {
    const { checkboxConfirm, close } = useDialogStore.getState();

    const promise = checkboxConfirm('Move to trash?', {
      checkbox: {
        label: 'Directly delete',
      },
    });

    close(false);

    const result = await promise;
    expect(result).toEqual({ confirmed: false, checked: false });
    expect(useDialogStore.getState().isOpen).toBe(false);
  });
});

describe('isAnyDialogOpen', () => {
  beforeEach(() => {
    useDialogStore.setState({
      isOpen: false,
      options: null,
      resolveRef: null,
      checkboxResolveRef: null,
    });
    useErrorDialogStore.setState({
      isOpen: false,
      options: null,
      resolveRef: null,
    });
  });

  it('returns false when both stores are closed', () => {
    expect(isAnyDialogOpen()).toBe(false);
  });

  it('returns true when useDialogStore is open and useErrorDialogStore is closed', () => {
    useDialogStore.setState({ isOpen: true });
    expect(isAnyDialogOpen()).toBe(true);
  });

  it('returns true when useErrorDialogStore is open and useDialogStore is closed', () => {
    useErrorDialogStore.setState({ isOpen: true });
    expect(isAnyDialogOpen()).toBe(true);
  });

  it('returns true when both stores are open', () => {
    useDialogStore.setState({ isOpen: true });
    useErrorDialogStore.setState({ isOpen: true });
    expect(isAnyDialogOpen()).toBe(true);
  });

  it('useIsAnyDialogOpen reactively tracks dialog open state', () => {
    const { result } = renderHook(() => useIsAnyDialogOpen());
    expect(result.current).toBe(false);

    act(() => {
      useDialogStore.setState({ isOpen: true });
    });
    expect(result.current).toBe(true);

    act(() => {
      useDialogStore.setState({ isOpen: false });
      useErrorDialogStore.setState({ isOpen: true });
    });
    expect(result.current).toBe(true);

    act(() => {
      useErrorDialogStore.setState({ isOpen: false });
    });
    expect(result.current).toBe(false);
  });
});
