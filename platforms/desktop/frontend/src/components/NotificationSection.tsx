import type React from 'react';
import type { NotificationEntry, NotificationPriority } from '../types/notification';
import { NotificationCard } from './NotificationCard';

interface NotificationSectionProps {
  priority: NotificationPriority;
  title: string;
  entries: NotificationEntry[];
}

export function NotificationSection({
  priority,
  title,
  entries,
}: NotificationSectionProps): React.JSX.Element | null {
  if (entries.length === 0) {
    return null;
  }

  return (
    <section className={`notification-center-section notification-center-section-${priority}`}>
      <div className="notification-center-section-title">
        {title}
        <span>{entries.length}</span>
      </div>
      <ul className="notification-center-list">
        {entries.map((entry) => (
          <NotificationCard key={entry.id} entry={entry} />
        ))}
      </ul>
    </section>
  );
}
