import type { NotificationAction } from '../../types/notification';

export type TFunction = (key: string, options?: Record<string, unknown>) => string;

/**
 * Maps the existing TaskCenterAction variant naming to the new NotificationAction
 * variant, normalizing 'secondarySoft' → 'soft'.
 */
export function toNotificationAction(action: {
  id: string;
  label: string;
  variant: 'primary' | 'secondary' | 'secondarySoft';
  disabled?: boolean;
  run: () => void | Promise<void>;
}): NotificationAction {
  return {
    id: action.id,
    label: action.label,
    variant: action.variant === 'secondarySoft' ? 'soft' : action.variant,
    disabled: action.disabled,
    run: action.run,
  };
}
