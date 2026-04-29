import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import { NotificationCenter } from '../NotificationCenter';

const recoveryState = {
  items: [] as any[],
  isLoaded: true,
};

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, options?: Record<string, unknown>) => {
      if (key === 'header.notifications') return 'Notifications';
      if (key === 'header.notifications_panel') return 'Notifications';
      if (key === 'header.notifications_empty') return 'No notifications right now.';
      if (key === 'recovery.banner.title') return 'Interrupted work is ready to recover';
      if (key === 'recovery.banner.body') {
        return `${options?.count} file(s) waiting. Batch: ${options?.batchCount} · Automation: ${options?.automationCount}`;
      }
      if (key === 'recovery.actions.open_center') return 'Open Recovery Center';
      return key;
    },
  }),
  initReactI18next: {
    type: '3rdParty',
    init: () => undefined,
  },
}));

vi.mock('../../stores/recoveryStore', () => ({
  useRecoveryStore: (selector: any) => selector(recoveryState),
}));

describe('NotificationCenter', () => {
  beforeEach(() => {
    recoveryState.items = [];
    recoveryState.isLoaded = true;
  });

  it('shows the empty state and no badge when there are no pending recovery items', () => {
    const { container } = render(<NotificationCenter onOpenRecoveryCenter={vi.fn()} />);

    expect(container.querySelector('.notification-center-trigger-badge')).toBeNull();

    fireEvent.click(screen.getByRole('button', { name: 'Notifications' }));

    expect(screen.getByRole('dialog', { name: 'Notifications' })).toBeDefined();
    expect(screen.getByText('No notifications right now.')).toBeDefined();
    expect(screen.queryByText('Interrupted work is ready to recover')).toBeNull();
  });

  it('renders one recovery notification with the aggregated counts', () => {
    recoveryState.items = [
      { id: 'recovery-1', source: 'batch_import', resolution: 'pending' },
      { id: 'recovery-2', source: 'batch_import', resolution: 'pending' },
      { id: 'recovery-3', source: 'automation', resolution: 'pending' },
    ];

    const { container } = render(<NotificationCenter onOpenRecoveryCenter={vi.fn()} />);

    expect(container.querySelector('.notification-center-trigger-badge')?.textContent).toBe('1');

    fireEvent.click(screen.getByRole('button', { name: 'Notifications' }));

    expect(screen.getByText('Interrupted work is ready to recover')).toBeDefined();
    expect(screen.getByText('3 file(s) waiting. Batch: 2 · Automation: 1')).toBeDefined();
    expect(screen.getByRole('button', { name: 'Open Recovery Center' })).toBeDefined();
  });

  it('opens the recovery center from either the notification body or the CTA', () => {
    recoveryState.items = [
      { id: 'recovery-1', source: 'batch_import', resolution: 'pending' },
    ];
    const onOpenRecoveryCenter = vi.fn();

    render(<NotificationCenter onOpenRecoveryCenter={onOpenRecoveryCenter} />);

    fireEvent.click(screen.getByRole('button', { name: 'Notifications' }));
    fireEvent.click(screen.getByRole('button', { name: /Interrupted work is ready to recover/i }));

    expect(onOpenRecoveryCenter).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole('dialog', { name: 'Notifications' })).toBeNull();

    fireEvent.click(screen.getByRole('button', { name: 'Notifications' }));
    fireEvent.click(screen.getByRole('button', { name: 'Open Recovery Center' }));

    expect(onOpenRecoveryCenter).toHaveBeenCalledTimes(2);
    expect(screen.queryByRole('dialog', { name: 'Notifications' })).toBeNull();
  });

  it('closes the open panel on outside click and Escape', () => {
    render(<NotificationCenter onOpenRecoveryCenter={vi.fn()} />);

    fireEvent.click(screen.getByRole('button', { name: 'Notifications' }));
    expect(screen.getByRole('dialog', { name: 'Notifications' })).toBeDefined();

    fireEvent.mouseDown(document.body);
    expect(screen.queryByRole('dialog', { name: 'Notifications' })).toBeNull();

    fireEvent.click(screen.getByRole('button', { name: 'Notifications' }));
    expect(screen.getByRole('dialog', { name: 'Notifications' })).toBeDefined();

    fireEvent.keyDown(document, { key: 'Escape' });
    expect(screen.queryByRole('dialog', { name: 'Notifications' })).toBeNull();
  });
});
