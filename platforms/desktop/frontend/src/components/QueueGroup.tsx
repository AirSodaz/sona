import type React from 'react';
import { useEffect, useState } from 'react';
import { ChevronDownIcon, ChevronRightIcon } from './Icons';
export interface QueueGroupProps {
  title: string;
  count: number;
  statusVariant?: 'processing' | 'pending' | 'complete' | 'failed';
  defaultExpanded?: boolean;
  action?: React.ReactNode;
  children: React.ReactNode;
}

/**
 * Collapsible section grouping queue items by processing status.
 */
export function QueueGroup({
  title,
  count,
  statusVariant = 'pending',
  defaultExpanded = true,
  action,
  children,
}: QueueGroupProps): React.JSX.Element | null {
  const [isExpanded, setIsExpanded] = useState(defaultExpanded);
  const [hasUserToggled, setHasUserToggled] = useState(false);

  useEffect(() => {
    if (!hasUserToggled) {
      setIsExpanded(defaultExpanded);
    }
  }, [defaultExpanded, hasUserToggled]);

  if (count === 0) return null;

  return (
    <div className={`queue-group queue-group-${statusVariant}`} role="group" aria-label={title}>
      <div className="queue-group-header">
        <button
          type="button"
          className="queue-group-expand-btn"
          onClick={() => {
            setHasUserToggled(true);
            setIsExpanded((prev) => !prev);
          }}
          aria-expanded={isExpanded}
        >
          <span className="queue-group-chevron" aria-hidden="true">
            {isExpanded ? (
              <ChevronDownIcon width={12} height={12} />
            ) : (
              <ChevronRightIcon width={12} height={12} />
            )}
          </span>
          <span className="queue-group-title">{title}</span>
          <span className="queue-group-badge">{count}</span>
        </button>

        {action && <div className="queue-group-action">{action}</div>}
      </div>

      {isExpanded && <div className="queue-group-items">{children}</div>}
    </div>
  );
}
