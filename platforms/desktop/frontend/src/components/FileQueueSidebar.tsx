import type React from 'react';
import { QueueGroupedList, type QueueGroupedListProps } from './QueueGroupedList';

export type FileQueueSidebarProps = QueueGroupedListProps;

/**
 * Compatibility wrapper re-exporting QueueGroupedList.
 */
export function FileQueueSidebar(props: FileQueueSidebarProps): React.JSX.Element | null {
  return <QueueGroupedList {...props} />;
}
