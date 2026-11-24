import { fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useDialogStore } from '../../stores/dialogStore';
import { GlobalDialog } from '../GlobalDialog';
import { Modal } from '../Modal';

vi.mock('react-i18next', async (importOriginal) => {
  const actual = await importOriginal<typeof import('react-i18next')>();

  return {
    ...actual,
    useTranslation: () => ({
      t: (_key: string, options?: { defaultValue?: string }) => options?.defaultValue ?? _key,
    }),
  };
});

describe('Modal', () => {
  beforeEach(() => {
    useDialogStore.setState({
      isOpen: false,
      options: null,
      resolveRef: null,
    });
  });

  it('wraps focus from the last enabled control when a disabled control follows it', () => {
    render(
      <Modal isOpen onClose={vi.fn()} title="Focusable Modal">
        <button type="button">First body action</button>
        <button type="button">Last enabled action</button>
        <button type="button" disabled>
          Disabled trailing action
        </button>
      </Modal>
    );

    const closeButton = screen.getByRole('button', { name: 'Close' });
    const lastEnabledButton = screen.getByRole('button', { name: 'Last enabled action' });

    lastEnabledButton.focus();
    fireEvent.keyDown(window, { key: 'Tab' });

    expect(document.activeElement).toBe(closeButton);
  });

  it('gives each modal instance a unique title id', () => {
    render(
      <>
        <Modal isOpen onClose={vi.fn()} title="First Modal">
          <button type="button">First action</button>
        </Modal>
        <Modal isOpen onClose={vi.fn()} title="Second Modal">
          <button type="button">Second action</button>
        </Modal>
      </>
    );

    const [firstDialog, secondDialog] = screen.getAllByRole('dialog');
    const firstTitleId = firstDialog.getAttribute('aria-labelledby');
    const secondTitleId = secondDialog.getAttribute('aria-labelledby');

    expect(firstTitleId).toBeTruthy();
    expect(secondTitleId).toBeTruthy();
    expect(firstTitleId).not.toBe(secondTitleId);
    expect(document.getElementById(firstTitleId!)?.textContent).toBe('First Modal');
    expect(document.getElementById(secondTitleId!)?.textContent).toBe('Second Modal');
  });

  it('lets the global dialog own Escape and visual stacking above shared modals', () => {
    const onClose = vi.fn();
    const resolve = vi.fn();
    useDialogStore.setState({
      isOpen: true,
      options: {
        message: 'Confirm the top-level action',
        type: 'alert',
      },
      resolveRef: resolve,
    });

    render(
      <>
        <Modal isOpen onClose={onClose} title="Base Modal">
          <button type="button">Base action</button>
        </Modal>
        <GlobalDialog />
      </>
    );

    const globalOverlay = screen.getByRole('alertdialog').parentElement;
    expect(Number(globalOverlay?.style.zIndex)).toBeGreaterThan(2100);

    fireEvent.keyDown(window, { key: 'Escape' });

    expect(onClose).not.toHaveBeenCalled();
    expect(resolve).toHaveBeenCalledTimes(1);
    expect(useDialogStore.getState().isOpen).toBe(false);
  });

  it('marks overlay with shared-modal-origin-settings when origin is settings', () => {
    render(
      <Modal isOpen onClose={vi.fn()} title="Settings Child Modal" origin="settings">
        <div>Child content</div>
      </Modal>
    );

    const overlay = document.querySelector('.shared-modal-overlay');
    expect(overlay?.classList.contains('shared-modal-origin-settings')).toBe(true);
  });

  it('marks overlay as nested when another overlay already exists in DOM', () => {
    const parentOverlay = document.createElement('div');
    parentOverlay.className = 'settings-overlay';
    document.body.appendChild(parentOverlay);

    try {
      render(
        <Modal isOpen onClose={vi.fn()} title="Nested Modal">
          <div>Nested content</div>
        </Modal>
      );

      const modalOverlay = document.querySelector('.shared-modal-overlay');
      expect(modalOverlay?.classList.contains('shared-modal-nested')).toBe(true);
    } finally {
      parentOverlay.remove();
    }
  });
});
