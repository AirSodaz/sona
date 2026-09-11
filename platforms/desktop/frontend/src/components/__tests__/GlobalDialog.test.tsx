import { render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useDialogStore } from '../../stores/dialogStore';
import { GlobalDialog } from '../GlobalDialog';

vi.mock('react-i18next', async (importOriginal) => {
  const actual = await importOriginal<typeof import('react-i18next')>();

  return {
    ...actual,
    useTranslation: () => ({
      t: (_key: string, options?: { defaultValue?: string }) => options?.defaultValue ?? _key,
    }),
  };
});

describe('GlobalDialog', () => {
  beforeEach(() => {
    useDialogStore.setState({
      isOpen: false,
      options: null,
      resolveRef: null,
    });
  });

  it('labels the AI auto-rename icon button and uses the standard tooltip', () => {
    useDialogStore.setState({
      isOpen: true,
      options: {
        message: 'Rename this transcript',
        type: 'prompt',
        onAiAction: vi.fn().mockResolvedValue('Generated title'),
      },
      resolveRef: vi.fn(),
    });

    render(<GlobalDialog />);

    const aiButton = screen.getByRole('button', { name: 'AI Auto-rename' });

    expect(aiButton.getAttribute('data-tooltip')).toBe('AI Auto-rename');
    expect(aiButton.getAttribute('data-tooltip-pos')).toBe('top');
    expect(aiButton.getAttribute('title')).toBeNull();
  });
});
