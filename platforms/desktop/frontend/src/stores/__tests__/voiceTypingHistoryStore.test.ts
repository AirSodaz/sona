// @vitest-environment jsdom
import { beforeEach, describe, expect, it } from 'vitest';
import { useVoiceTypingHistoryStore } from '../voiceTypingHistoryStore';

describe('voiceTypingHistoryStore', () => {
  beforeEach(() => {
    localStorage.clear();
    useVoiceTypingHistoryStore.getState().clearHistory();
  });

  it('adds an item and persists to storage', () => {
    const store = useVoiceTypingHistoryStore.getState();
    const item = store.addItem({
      rawText: '原始语音',
      injectedText: '原始语音',
      mode: 'raw',
    });

    expect(item.id).toBeTruthy();
    expect(item.rawText).toBe('原始语音');
    expect(useVoiceTypingHistoryStore.getState().items).toHaveLength(1);
    expect(localStorage.getItem('sona:voice-typing:history')).toContain('原始语音');
  });

  it('removes an item by id', () => {
    const store = useVoiceTypingHistoryStore.getState();
    const item1 = store.addItem({ rawText: '条目1', injectedText: '条目1', mode: 'raw' });
    const item2 = store.addItem({ rawText: '条目2', injectedText: '条目2', mode: 'polish' });

    expect(useVoiceTypingHistoryStore.getState().items).toHaveLength(2);

    useVoiceTypingHistoryStore.getState().removeItem(item1.id);
    const remaining = useVoiceTypingHistoryStore.getState().items;

    expect(remaining).toHaveLength(1);
    expect(remaining[0].id).toBe(item2.id);
  });

  it('clears all history', () => {
    const store = useVoiceTypingHistoryStore.getState();
    store.addItem({ rawText: '测试', injectedText: '测试', mode: 'raw' });

    useVoiceTypingHistoryStore.getState().clearHistory();

    expect(useVoiceTypingHistoryStore.getState().items).toHaveLength(0);
    expect(localStorage.getItem('sona:voice-typing:history')).toBe('[]');
  });
});
