import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { ErrorDialog } from '../ErrorDialog';
import { useErrorDialogStore } from '../../stores/errorDialogStore';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, options?: Record<string, unknown>) => String(options?.defaultValue ?? key),
  }),
}));

describe('ErrorDialog', () => {
  beforeEach(() => {
    useErrorDialogStore.setState({
      isOpen: false,
      options: null,
      resolveRef: null,
    });
  });

  it('renders details and closes with Escape', async () => {
    const resolveRef = vi.fn();
    useErrorDialogStore.setState({
      isOpen: true,
      options: {
        title: 'Error',
        message: 'Update failed.',
        details: 'network timeout',
        primaryLabel: 'OK',
        hasPrimaryAction: false,
      },
      resolveRef,
    });

    render(<ErrorDialog />);

    expect(screen.getByRole('alertdialog')).toBeDefined();
    expect(screen.getByText('Update failed.')).toBeDefined();
    expect(screen.getByText('network timeout')).toBeDefined();

    fireEvent.keyDown(window, { key: 'Escape' });

    expect(useErrorDialogStore.getState().isOpen).toBe(false);
  });

  it('focuses cancel first for actionable errors', async () => {
    useErrorDialogStore.setState({
      isOpen: true,
      options: {
        title: 'Error',
        message: 'Update failed.',
        details: 'network timeout',
        primaryLabel: 'Download Manually',
        cancelLabel: 'Cancel',
        hasPrimaryAction: true,
      },
      resolveRef: vi.fn(),
    });

    render(<ErrorDialog />);

    await waitFor(() => expect((document.activeElement as HTMLElement | null)?.textContent).toBe('Cancel'));
  });

  it('resolves the primary action for actionable errors', () => {
    useErrorDialogStore.setState({
      isOpen: true,
      options: {
        title: 'Error',
        message: 'Update failed.',
        details: 'network timeout',
        primaryLabel: 'Download Manually',
        cancelLabel: 'Cancel',
        hasPrimaryAction: true,
      },
      resolveRef: vi.fn(),
    });

    render(<ErrorDialog />);

    fireEvent.click(screen.getByText('Download Manually'));

    expect(useErrorDialogStore.getState().isOpen).toBe(false);
  });
});
