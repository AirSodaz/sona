import { fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useDialogStore } from '../../stores/dialogStore';
import { GlobalDialog } from '../GlobalDialog';

vi.mock('react-i18next', async (importOriginal) => {
  const actual = await importOriginal<typeof import('react-i18next')>();

  return {
    ...actual,
    useTranslation: () => ({
      t: (_key: string, options?: { defaultValue?: string }) => options?.defaultValue ?? _key,
    }),
  };
});

describe('GlobalDialog', () => {
  beforeEach(() => {
    useDialogStore.setState({
      isOpen: false,
      options: null,
      resolveRef: null,
      checkboxResolveRef: null,
    });
  });

  it('labels the AI auto-rename icon button and uses the standard tooltip', () => {
    useDialogStore.setState({
      isOpen: true,
      options: {
        message: 'Rename this transcript',
        type: 'prompt',
        onAiAction: vi.fn().mockResolvedValue('Generated title'),
      },
      resolveRef: vi.fn(),
    });

    render(<GlobalDialog />);

    const aiButton = screen.getByRole('button', { name: 'AI Auto-rename' });

    expect(aiButton.getAttribute('data-tooltip')).toBe('AI Auto-rename');
    expect(aiButton.getAttribute('data-tooltip-pos')).toBe('top');
    expect(aiButton.getAttribute('title')).toBeNull();
  });

  it('renders checkbox and updates confirm button label and notice when toggled', () => {
    const closeCheckboxConfirmSpy = vi.spyOn(useDialogStore.getState(), 'closeCheckboxConfirm');
    useDialogStore.setState({
      isOpen: true,
      options: {
        message: 'Move this item to Trash?',
        type: 'confirm',
        confirmLabel: 'Move to Trash',
        cancelLabel: 'Cancel',
        checkbox: {
          label: 'Directly delete permanently (bypass Trash)',
          checkedConfirmLabel: 'Delete Permanently',
          checkedNotice: 'This will permanently delete the records and cannot be undone.',
        },
      },
    });

    render(<GlobalDialog />);

    expect(screen.getByText('Move this item to Trash?')).toBeDefined();
    expect(screen.getByText('Directly delete permanently (bypass Trash)')).toBeDefined();
    expect(screen.getByRole('button', { name: 'Move to Trash' })).toBeDefined();
    expect(
      screen.queryByText('This will permanently delete the records and cannot be undone.')
    ).toBeNull();

    // Toggle checkbox on
    const checkboxElement = screen.getByRole('checkbox', {
      name: 'Directly delete permanently (bypass Trash)',
    });
    fireEvent.click(checkboxElement);

    // Notice should now be visible
    expect(
      screen.getByText('This will permanently delete the records and cannot be undone.')
    ).toBeDefined();
    // Confirm button label should have updated
    const confirmBtn = screen.getByRole('button', { name: 'Delete Permanently' });
    expect(confirmBtn).toBeDefined();

    // Clicking confirm should call closeCheckboxConfirm with { confirmed: true, checked: true }
    fireEvent.click(confirmBtn);
    expect(closeCheckboxConfirmSpy).toHaveBeenCalledWith({ confirmed: true, checked: true });
  });

  it('calls closeCheckboxConfirm with { confirmed: false, checked: false } when cancelled', () => {
    const closeCheckboxConfirmSpy = vi.spyOn(useDialogStore.getState(), 'closeCheckboxConfirm');
    useDialogStore.setState({
      isOpen: true,
      options: {
        message: 'Move this item to Trash?',
        type: 'confirm',
        confirmLabel: 'Move to Trash',
        cancelLabel: 'Cancel',
        checkbox: {
          label: 'Directly delete permanently (bypass Trash)',
        },
      },
    });

    render(<GlobalDialog />);

    const cancelBtn = screen.getByRole('button', { name: 'Cancel' });
    fireEvent.click(cancelBtn);
    expect(closeCheckboxConfirmSpy).toHaveBeenCalledWith({ confirmed: false, checked: false });
  });
});
