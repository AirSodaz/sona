vi.mock('@tauri-apps/api/core', () => ({
  convertFileSrc: vi.fn((path) => `asset://${path}`),
  invoke: vi.fn().mockResolvedValue(undefined),
}));

vi.mock('../../../services/tauri/invoke', () => ({
  invokeTauri: vi.fn().mockResolvedValue(undefined),
}));

import { fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useBatchQueueStore } from '../../../stores/batchQueueStore';
import { UrlStreamImportModal } from '../UrlStreamImportModal';

describe('UrlStreamImportModal', () => {
  beforeEach(() => {
    useBatchQueueStore.setState({ queueItems: [] });
  });

  it('renders modal when open and validates input', () => {
    const handleClose = vi.fn();
    render(<UrlStreamImportModal isOpen={true} onClose={handleClose} />);

    expect(screen.getByText('导入网络音视频链接')).toBeDefined();

    const textarea = screen.getByRole('textbox');
    fireEvent.change(textarea, { target: { value: 'invalid-url' } });

    const submitBtn = screen.getByText('加入队列');
    fireEvent.click(submitBtn);

    expect(
      screen.getByText('链接必须以 http://, https://, rtmp:// 或 rtsp:// 开头。')
    ).toBeDefined();
  });

  it('submits valid stream URLs and enqueues items', () => {
    const handleClose = vi.fn();
    render(<UrlStreamImportModal isOpen={true} onClose={handleClose} />);

    const textarea = screen.getByPlaceholderText(/https:\/\/example.com\/podcast.mp3/);
    fireEvent.change(textarea, {
      target: { value: 'https://example.com/audio/podcast.mp3' },
    });

    const aliasInput = screen.getByPlaceholderText(/例如：AI 播客第 42 期/);
    fireEvent.change(aliasInput, { target: { value: 'My Stream Episode' } });

    const retainCheckbox = screen.getByRole('checkbox');
    fireEvent.click(retainCheckbox);

    const submitBtn = screen.getByText('加入队列');
    fireEvent.click(submitBtn);

    expect(handleClose).toHaveBeenCalled();
    const items = useBatchQueueStore.getState().queueItems;
    expect(items).toHaveLength(1);
    expect(items[0].sourceType).toBe('url_stream');
    expect(items[0].streamUrl).toBe('https://example.com/audio/podcast.mp3');
    expect(items[0].filename).toBe('My Stream Episode');
    expect(items[0].retainAudio).toBe(true);
  });
});
