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
    expect(dialog.classList.contains('panel-modal-shell')).toBe(true);
    expect(dialog.classList.contains('test-modal')).toBe(true);
    expect(dialog.querySelector('.panel-modal-header.test-header')).toBeTruthy();
    expect(dialog.querySelector('.panel-modal-header-copy.test-header-copy')).toBeTruthy();
    expect(dialog.querySelector('.panel-modal-header-controls.test-header-controls')).toBeTruthy();
    expect(dialog.querySelector('.panel-modal-toolbar.test-toolbar')).toBeTruthy();
    expect(dialog.querySelector('.panel-modal-badge.test-badge')).toBeTruthy();
    expect(dialog.querySelector('.panel-modal-meta-row.test-meta')).toBeTruthy();
    expect(dialog.querySelector('.panel-modal-content.test-content')).toBeTruthy();
    expect(screen.getByText('Badge')).toBeDefined();
    expect(screen.getByText('Test Title')).toBeDefined();
    expect(screen.getByText('Test description')).toBeDefined();
    expect(screen.getByText('Action')).toBeDefined();
    expect(screen.getByText('Meta Label')).toBeDefined();
    expect(screen.getByText('Meta Value')).toBeDefined();
    expect(screen.getByRole('alert').textContent).toContain('Banner error');
    expect(screen.getByText('Body')).toBeDefined();
    expect(screen.getByRole('button', { name: 'Close' })).toBeDefined();
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

    fireEvent.click(screen.getByRole('dialog', { name: 'Test Panel' }));
    expect(onClose).not.toHaveBeenCalled();

    const overlay = document.querySelector('.panel-modal-overlay.test-overlay') as HTMLElement;
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
});
