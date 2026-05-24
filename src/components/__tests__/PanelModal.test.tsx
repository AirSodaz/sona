import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { PanelModal } from '../PanelModal';

describe('PanelModal', () => {
  it('does not render when closed', () => {
    const { container } = render(
      <PanelModal
        isOpen={false}
        onClose={vi.fn()}
        ariaLabel="Test Panel"
        title="Test Title"
      >
        <div>Body</div>
      </PanelModal>,
    );

    expect(container.firstChild).toBeNull();
  });

  it('renders shared modal shell slots and close button', () => {
    render(
      <PanelModal
        isOpen
        onClose={vi.fn()}
        ariaLabel="Test Panel"
        size="settings"
        origin="settings"
        onBack={vi.fn()}
        backLabel="Back"
        className="test-modal"
        overlayClassName="test-overlay"
        headerClassName="test-header"
        headerCopyClassName="test-header-copy"
        headerControlsClassName="test-header-controls"
        toolbarClassName="test-toolbar"
        badgeClassName="test-badge"
        metaClassName="test-meta"
        contentClassName="test-content"
        badge={<span>Badge</span>}
        title="Test Title"
        description="Test description"
        headerActions={<button type="button">Action</button>}
        meta={<><span>Meta Label</span><span>Meta Value</span></>}
        errorBanner={<div role="alert">Banner error</div>}
      >
        <div>Body</div>
      </PanelModal>,
    );

    const dialog = screen.getByRole('dialog', { name: 'Test Panel' });
    const overlay = document.querySelector('.panel-modal-overlay.test-overlay') as HTMLElement | null;
    expect(dialog.classList.contains('panel-modal-shell')).toBe(true);
    expect(dialog.classList.contains('test-modal')).toBe(true);
    expect(dialog.classList.contains('panel-modal-size-settings')).toBe(true);
    const header = dialog.querySelector('.panel-modal-header.test-header');
    expect(header).toBeTruthy();
    const topRow = dialog.querySelector('.panel-modal-top-row');
    expect(topRow).toBeTruthy();
    expect(dialog.querySelector('.panel-modal-header-leading')).toBeTruthy();
    expect(dialog.querySelector('.panel-modal-header-copy.test-header-copy')).toBeTruthy();
    expect(dialog.querySelector('.panel-modal-header-controls.test-header-controls')).toBeTruthy();
    expect(topRow?.contains(screen.getByRole('button', { name: 'Back' }))).toBe(true);
    const toolbar = dialog.querySelector('.panel-modal-toolbar.test-toolbar');
    const closeButton = screen.getByRole('button', { name: 'Close' });
    expect(toolbar).toBeTruthy();
    expect(topRow?.contains(toolbar)).toBe(true);
    expect(topRow?.contains(closeButton)).toBe(true);
    expect(topRow?.contains(screen.getByText('Badge'))).toBe(true);
    expect(toolbar?.contains(closeButton)).toBe(false);
    expect(dialog.querySelector('.panel-modal-close-slot .panel-modal-close')).toBe(closeButton);
    expect(dialog.querySelector('.panel-modal-badge.test-badge')).toBeTruthy();
    expect(dialog.querySelector('.panel-modal-header-copy .panel-modal-badge')).toBeNull();
    expect(dialog.querySelector('.panel-modal-meta-row.test-meta')).toBeTruthy();
    expect(dialog.querySelector('.panel-modal-content.test-content')).toBeTruthy();
    expect(overlay?.classList.contains('panel-modal-origin-settings')).toBe(true);
    expect(overlay?.classList.contains('settings-overlay')).toBe(false);
    expect(screen.getByRole('button', { name: 'Back' })).toBeDefined();
    expect(screen.queryByText('Back')).toBeNull();
    expect(screen.getByText('Badge')).toBeDefined();
    expect(screen.getByText('Test Title')).toBeDefined();
    expect(screen.getByText('Test description')).toBeDefined();
    expect(screen.getByText('Action')).toBeDefined();
    expect(screen.getByText('Meta Label')).toBeDefined();
    expect(screen.getByText('Meta Value')).toBeDefined();
    expect(screen.getByRole('alert').textContent).toContain('Banner error');
    expect(screen.getByText('Body')).toBeDefined();
    expect(closeButton).toBeDefined();
  });

  it('closes on overlay click but not on shell click', () => {
    const onClose = vi.fn();
    render(
      <PanelModal
        isOpen
        onClose={onClose}
        ariaLabel="Test Panel"
        title="Test Title"
        overlayClassName="test-overlay"
      >
        <div>Body</div>
      </PanelModal>,
    );

    const overlay = document.querySelector('.panel-modal-overlay.test-overlay') as HTMLElement;
    expect(overlay.classList.contains('panel-modal-origin-standalone')).toBe(true);
    expect(overlay.classList.contains('settings-overlay')).toBe(false);

    fireEvent.click(screen.getByRole('dialog', { name: 'Test Panel' }));
    expect(onClose).not.toHaveBeenCalled();

    fireEvent.click(overlay);
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('closes when the close button is pressed', () => {
    const onClose = vi.fn();
    render(
      <PanelModal
        isOpen
        onClose={onClose}
        ariaLabel="Test Panel"
        title="Test Title"
      >
        <div>Body</div>
      </PanelModal>,
    );

    fireEvent.click(screen.getByRole('button', { name: 'Close' }));
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('calls back for settings-origin panels and hides back button for standalone panels', () => {
    const onBack = vi.fn();
    const { rerender } = render(
      <PanelModal
        isOpen
        onClose={vi.fn()}
        ariaLabel="Settings Panel"
        title="Settings Title"
        origin="settings"
        onBack={onBack}
        backLabel="Go Back"
      >
        <div>Body</div>
      </PanelModal>,
    );

    fireEvent.click(screen.getByRole('button', { name: 'Go Back' }));
    expect(onBack).toHaveBeenCalledTimes(1);

    rerender(
      <PanelModal
        isOpen
        onClose={vi.fn()}
        ariaLabel="Standalone Panel"
        title="Standalone Title"
        origin="standalone"
      >
        <div>Body</div>
      </PanelModal>,
    );

    expect(screen.queryByRole('button', { name: 'Go Back' })).toBeNull();
  });
});
