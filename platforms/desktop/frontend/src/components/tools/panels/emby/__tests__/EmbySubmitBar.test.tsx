import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import type * as ReactI18Next from 'react-i18next';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { EmbySubmitBar } from '../EmbySubmitBar';

vi.mock('react-i18next', async (importOriginal) => {
  const actual = await importOriginal<typeof ReactI18Next>();
  return {
    ...actual,
    useTranslation: () => ({
      t: (key: string, opts?: { defaultValue?: string; count?: number }) => {
        if (key === 'emby.submit_btn_compact') {
          return `Transcribe (${opts?.count ?? 0})`;
        }
        return opts?.defaultValue ?? key;
      },
      i18n: { language: 'en' },
    }),
  };
});

describe('EmbySubmitBar', () => {
  afterEach(() => {
    cleanup();
  });

  it('renders compact submit button with count and disables it when count is 0', () => {
    const onSubmit = vi.fn();
    render(
      <EmbySubmitBar
        selectedCount={0}
        subtitleMode="bilingual"
        onSubtitleModeChange={vi.fn()}
        overwriteExistingExternal={false}
        onOverwriteExistingExternalChange={vi.fn()}
        onSubmitBatch={onSubmit}
      />
    );

    const submitBtn = screen.getByRole('button', { name: /Transcribe \(0\)/i });
    expect(submitBtn).toBeDefined();
    expect(submitBtn.hasAttribute('disabled')).toBe(true);

    // Verify clear selection button does not exist in document
    expect(screen.queryByRole('button', { name: /Clear Selection/i })).toBeNull();
  });

  it('enables submit button when selectedCount > 0 and calls onSubmitBatch on click', () => {
    const onSubmit = vi.fn();
    render(
      <EmbySubmitBar
        selectedCount={5}
        subtitleMode="bilingual"
        onSubtitleModeChange={vi.fn()}
        overwriteExistingExternal={false}
        onOverwriteExistingExternalChange={vi.fn()}
        onSubmitBatch={onSubmit}
      />
    );

    const submitBtn = screen.getByRole('button', { name: /Transcribe \(5\)/i });
    expect(submitBtn.hasAttribute('disabled')).toBe(false);

    fireEvent.click(submitBtn);
    expect(onSubmit).toHaveBeenCalledTimes(1);

    // Verify clear selection button still does not exist
    expect(screen.queryByRole('button', { name: /Clear Selection/i })).toBeNull();
  });

  it('disables submit button while submitting', () => {
    const onSubmit = vi.fn();
    render(
      <EmbySubmitBar
        selectedCount={3}
        subtitleMode="pure_translation"
        onSubtitleModeChange={vi.fn()}
        overwriteExistingExternal={true}
        onOverwriteExistingExternalChange={vi.fn()}
        onSubmitBatch={onSubmit}
        isSubmitting={true}
      />
    );

    const submitBtn = screen.getByRole('button', { name: /Transcribe \(3\)/i });
    expect(submitBtn.hasAttribute('disabled')).toBe(true);
  });

  it('renders submitted notice message when provided', () => {
    render(
      <EmbySubmitBar
        selectedCount={0}
        subtitleMode="bilingual"
        onSubtitleModeChange={vi.fn()}
        overwriteExistingExternal={false}
        onOverwriteExistingExternalChange={vi.fn()}
        onSubmitBatch={vi.fn()}
        submittedNotice="Added 3 items to queue"
      />
    );

    expect(screen.getByText('Added 3 items to queue')).toBeDefined();
  });
});
