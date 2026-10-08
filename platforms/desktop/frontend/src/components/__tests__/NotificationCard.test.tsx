import { act, fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { NotificationEntry } from '../../types/notification';
import { NotificationCard } from '../NotificationCard';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, options?: { defaultValue?: string }) => {
      const translations: Record<string, string> = {
        'task_center.expand': 'Show more',
        'task_center.collapse': 'Show less',
        'task_center.expand_detail': 'Show details',
        'task_center.collapse_detail': 'Hide details',
        'task_center.copy_detail': 'Copy details',
        'task_center.copied': 'Copied',
        'task_center.progress': 'Progress',
      };
      return translations[key] ?? options?.defaultValue ?? key;
    },
  }),
}));

function createMockEntry(overrides?: Partial<NotificationEntry>): NotificationEntry {
  return {
    id: 'test-notification',
    source: 'task',
    priority: 'info',
    tone: 'info',
    icon: <span data-testid="test-icon">icon</span>,
    title: 'Test Notification',
    body: 'Short message',
    timestamp: Date.now(),
    actions: [],
    ...overrides,
  };
}

describe('NotificationCard Collapsible Feature', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders short notifications normally without expand/collapse toggles', () => {
    const entry = createMockEntry({
      body: 'Everything finished successfully.',
    });

    const { container } = render(<NotificationCard entry={entry} />);

    expect(screen.getByText('Test Notification')).toBeDefined();
    expect(screen.getByText('Everything finished successfully.')).toBeDefined();

    // No toggle buttons should be present
    expect(container.querySelector('.notification-center-expand-btn')).toBeNull();
    expect(container.querySelector('.notification-center-item-toggle')).toBeNull();
    expect(container.querySelector('.notification-center-item-collapsible')).toBeNull();
  });

  it('collapses long body text by default and expands on click', () => {
    const longChangelog = [
      'v1.0.0 Release Notes:',
      '- Added Notification Center collapse functionality',
      '- Improved audio transcribing throughput by 40%',
      '- Fixed memory management in worker background thread',
      '- Upgraded design tokens and dark mode contrast',
    ].join('\n');

    const entry = createMockEntry({
      body: longChangelog,
      bodyClassName: 'notification-center-update-body',
    });

    const { container } = render(<NotificationCard entry={entry} />);

    // Starts collapsed
    const item = container.querySelector('.notification-center-item');
    expect(item?.classList.contains('notification-center-item-collapsible')).toBe(true);
    expect(item?.classList.contains('is-collapsed')).toBe(true);
    expect(item?.classList.contains('is-expanded')).toBe(false);

    // Expand toggle button is visible
    const inlineExpandBtn = container.querySelector(
      '.notification-center-expand-btn'
    ) as HTMLButtonElement;
    expect(inlineExpandBtn).not.toBeNull();
    expect(inlineExpandBtn.textContent).toContain('Show more');
    expect(inlineExpandBtn.getAttribute('aria-expanded')).toBe('false');

    // Header toggle button is no longer present
    expect(container.querySelector('.notification-center-item-toggle')).toBeNull();

    // Click inline expand button
    fireEvent.click(inlineExpandBtn);

    // Now expanded
    expect(item?.classList.contains('is-expanded')).toBe(true);
    expect(item?.classList.contains('is-collapsed')).toBe(false);
    expect(inlineExpandBtn.textContent).toContain('Show less');
    expect(inlineExpandBtn.getAttribute('aria-expanded')).toBe('true');

    // Click collapse button
    fireEvent.click(inlineExpandBtn);

    // Collapsed again
    expect(item?.classList.contains('is-collapsed')).toBe(true);
    expect(item?.classList.contains('is-expanded')).toBe(false);
    expect(inlineExpandBtn.textContent).toContain('Show more');
  });

  it('collapses long error details and provides expand/collapse and copy functionality', async () => {
    const longStackTrace = [
      'ConnectionError: HTTPSConnectionPool(host="api.openai.com", port=443)',
      'Traceback (most recent call last):',
      '  File "service/pipeline.py", line 142, in process_batch',
      '    response = client.chat.completions.create(...)',
      'ConnectionRefusedError: [Errno 111] Connection refused',
    ].join('\n');

    const entry = createMockEntry({
      tone: 'error',
      detail: longStackTrace,
    });

    const writeTextMock = vi.fn().mockResolvedValue(undefined);
    Object.assign(navigator, {
      clipboard: {
        writeText: writeTextMock,
      },
    });

    const { container } = render(<NotificationCard entry={entry} />);

    // Starts with collapsed detail
    const detailBox = container.querySelector('.notification-center-item-detail-box');
    expect(detailBox?.classList.contains('is-collapsible')).toBe(true);
    expect(detailBox?.classList.contains('is-collapsed')).toBe(true);

    const detailText = container.querySelector('.notification-center-item-detail');
    expect(detailText?.classList.contains('is-collapsed')).toBe(true);

    // Expand detail
    const expandDetailBtn = screen.getByRole('button', { name: 'Show details' });
    fireEvent.click(expandDetailBtn);

    expect(detailBox?.classList.contains('is-expanded')).toBe(true);
    expect(detailText?.classList.contains('is-collapsed')).toBe(false);
    expect(screen.getByRole('button', { name: 'Hide details' })).toBeDefined();

    // Copy detail button works
    // Copy detail button works and does NOT have a tooltip (neither title nor data-tooltip)
    const copyBtn = screen.getByRole('button', { name: 'Copy details' });
    expect(copyBtn.getAttribute('title')).toBeNull();
    expect(copyBtn.getAttribute('data-tooltip')).toBeNull();
    await act(async () => {
      fireEvent.click(copyBtn);
    });

    expect(writeTextMock).toHaveBeenCalledWith(longStackTrace);
    expect(screen.getByText('Copied')).toBeDefined();
  });

  it('renders close button with unified project data-tooltip and no native title', () => {
    const onClose = vi.fn();
    const entry = createMockEntry({
      closeAction: {
        id: 'dismiss',
        label: 'Dismiss notification',
        variant: 'soft',
        run: onClose,
      },
    });

    const { container } = render(<NotificationCard entry={entry} />);
    const closeBtn = container.querySelector(
      '.notification-center-item-close'
    ) as HTMLButtonElement;
    expect(closeBtn).not.toBeNull();
    expect(closeBtn.getAttribute('data-tooltip')).toBe('Dismiss notification');
    expect(closeBtn.getAttribute('data-tooltip-pos')).toBe('left');
    expect(closeBtn.getAttribute('title')).toBeNull();

    fireEvent.click(closeBtn);
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('respects defaultExpanded prop when true', () => {
    const entry = createMockEntry({
      body: 'Line 1\nLine 2\nLine 3\nLine 4',
      defaultExpanded: true,
    });

    const { container } = render(<NotificationCard entry={entry} />);
    const item = container.querySelector('.notification-center-item');

    expect(item?.classList.contains('is-expanded')).toBe(true);
    const inlineExpandBtn = container.querySelector('.notification-center-expand-btn');
    expect(inlineExpandBtn?.textContent).toContain('Show less');
  });

  it('allows clicking onOpen without interference from the expand toggle', () => {
    const onOpen = vi.fn();
    const entry = createMockEntry({
      body: 'Long message line 1\nLong message line 2\nLong message line 3\nLong message line 4',
      onOpen,
    });

    const { container } = render(<NotificationCard entry={entry} />);

    // Clicking the title area triggers onOpen
    fireEvent.click(screen.getByText('Test Notification'));
    expect(onOpen).toHaveBeenCalledTimes(1);

    // Clicking the expand toggle does NOT trigger onOpen (due to stopPropagation)
    const expandBtn = container.querySelector(
      '.notification-center-expand-btn'
    ) as HTMLButtonElement;
    fireEvent.click(expandBtn);
    expect(onOpen).toHaveBeenCalledTimes(1); // Still 1, not incremented!
  });

  it('allows keyboard navigation on main card and prevents bubbling from expand toggle', () => {
    const onOpen = vi.fn();
    const entry = createMockEntry({
      body: 'Long message line 1\nLong message line 2\nLong message line 3\nLong message line 4',
      onOpen,
    });

    const { container } = render(<NotificationCard entry={entry} />);
    const mainAction = screen.getByRole('button', { name: /Test Notification/ });

    // Pressing Enter on the main card triggers onOpen
    fireEvent.keyDown(mainAction, { key: 'Enter' });
    expect(onOpen).toHaveBeenCalledTimes(1);

    // Pressing Space on the main card triggers onOpen
    fireEvent.keyDown(mainAction, { key: ' ' });
    expect(onOpen).toHaveBeenCalledTimes(2);

    const expandBtn = container.querySelector(
      '.notification-center-expand-btn'
    ) as HTMLButtonElement;

    // Pressing Enter or Space on the expand toggle does NOT trigger onOpen
    fireEvent.keyDown(expandBtn, { key: 'Enter' });
    expect(onOpen).toHaveBeenCalledTimes(2);

    fireEvent.keyDown(expandBtn, { key: ' ' });
    expect(onOpen).toHaveBeenCalledTimes(2);
  });

  it('expands and collapses body and detail independently', () => {
    const entry = createMockEntry({
      body: 'Line 1\nLine 2\nLine 3\nLine 4',
      detail: 'Detail line 1\nDetail line 2\nDetail line 3',
    });

    const { container } = render(<NotificationCard entry={entry} />);

    const bodySpan = container.querySelector('.notification-center-item-body');
    const detailBox = container.querySelector('.notification-center-item-detail-box');
    expect(bodySpan?.classList.contains('is-collapsed')).toBe(true);
    expect(detailBox?.classList.contains('is-collapsed')).toBe(true);

    // Expand body
    const expandBodyBtn = screen.getByRole('button', { name: 'Show more' });
    fireEvent.click(expandBodyBtn);

    expect(bodySpan?.classList.contains('is-expanded')).toBe(true);
    // Detail remains collapsed
    expect(detailBox?.classList.contains('is-collapsed')).toBe(true);

    // Expand detail
    const expandDetailBtn = screen.getByRole('button', { name: 'Show details' });
    fireEvent.click(expandDetailBtn);

    expect(bodySpan?.classList.contains('is-expanded')).toBe(true);
    expect(detailBox?.classList.contains('is-expanded')).toBe(true);

    // Collapse body
    const collapseBodyBtn = screen.getByRole('button', { name: 'Show less' });
    fireEvent.click(collapseBodyBtn);

    expect(bodySpan?.classList.contains('is-collapsed')).toBe(true);
    // Detail remains expanded
    expect(detailBox?.classList.contains('is-expanded')).toBe(true);
  });

  it('shows copy button for short error details without collapse toggle', async () => {
    const writeTextMock = vi.fn().mockResolvedValue(undefined);
    Object.assign(navigator, {
      clipboard: {
        writeText: writeTextMock,
      },
    });

    const entry = createMockEntry({
      tone: 'error',
      detail: 'Network timeout',
    });

    const { container } = render(<NotificationCard entry={entry} />);

    // No collapse button
    expect(container.querySelector('.notification-center-expand-btn')).toBeNull();
    expect(
      container
        .querySelector('.notification-center-item-detail-box')
        ?.classList.contains('is-collapsible')
    ).toBe(false);

    // Copy button is present because tone is error
    const copyBtn = screen.getByRole('button', { name: 'Copy details' });
    expect(copyBtn).not.toBeNull();

    await act(async () => {
      fireEvent.click(copyBtn);
    });

    expect(writeTextMock).toHaveBeenCalledWith('Network timeout');
  });

  it('hides both copy and collapse buttons for short non-error details', () => {
    const entry = createMockEntry({
      tone: 'info',
      detail: 'Stops after the current step and skips the final writeback.',
    });

    const { container } = render(<NotificationCard entry={entry} />);

    expect(
      screen.getByText('Stops after the current step and skips the final writeback.')
    ).toBeDefined();
    expect(container.querySelector('.notification-center-expand-btn')).toBeNull();
    expect(container.querySelector('.notification-center-detail-copy-btn')).toBeNull();
    expect(container.querySelector('.notification-center-detail-actions')).toBeNull();
  });

  it('handles model download error entry structure correctly', async () => {
    const writeTextMock = vi.fn().mockResolvedValue(undefined);
    Object.assign(navigator, {
      clipboard: {
        writeText: writeTextMock,
      },
    });

    const entry = createMockEntry({
      id: 'onboarding-download',
      source: 'download',
      tone: 'error',
      title: 'Model download failed',
      body: 'Could not finish downloading the recommended models.',
      detail: 'Failed to connect to huggingface.co: connection refused',
    });

    render(<NotificationCard entry={entry} />);

    expect(screen.getByText('Model download failed')).toBeDefined();
    expect(screen.getByText('Could not finish downloading the recommended models.')).toBeDefined();
    expect(
      screen.getByText('Failed to connect to huggingface.co: connection refused')
    ).toBeDefined();

    const copyBtn = screen.getByRole('button', { name: 'Copy details' });
    await act(async () => {
      fireEvent.click(copyBtn);
    });
    expect(writeTextMock).toHaveBeenCalledWith(
      'Failed to connect to huggingface.co: connection refused'
    );
  });
});
