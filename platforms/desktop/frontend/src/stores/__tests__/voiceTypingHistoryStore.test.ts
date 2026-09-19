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
      rawText: 'Raw speech input',
      injectedText: 'Raw speech input',
      mode: 'raw',
    });

    expect(item.id).toBeTruthy();
    expect(item.rawText).toBe('Raw speech input');
    expect(useVoiceTypingHistoryStore.getState().items).toHaveLength(1);
    expect(localStorage.getItem('sona:voice-typing:history')).toContain('Raw speech input');
  });

  it('removes an item by id', () => {
    const store = useVoiceTypingHistoryStore.getState();
    const item1 = store.addItem({ rawText: 'Entry 1', injectedText: 'Entry 1', mode: 'raw' });
    const item2 = store.addItem({ rawText: 'Entry 2', injectedText: 'Entry 2', mode: 'polish' });

    expect(useVoiceTypingHistoryStore.getState().items).toHaveLength(2);

    useVoiceTypingHistoryStore.getState().removeItem(item1.id);
    const remaining = useVoiceTypingHistoryStore.getState().items;

    expect(remaining).toHaveLength(1);
    expect(remaining[0].id).toBe(item2.id);
  });

  it('clears all history', () => {
    const store = useVoiceTypingHistoryStore.getState();
    store.addItem({ rawText: 'Test text', injectedText: 'Test text', mode: 'raw' });

    useVoiceTypingHistoryStore.getState().clearHistory();

    expect(useVoiceTypingHistoryStore.getState().items).toHaveLength(0);
    expect(localStorage.getItem('sona:voice-typing:history')).toBe('[]');
  });

  it('reloads history from storage and responds to storage events', () => {
    const externalItems = [
      {
        id: 'ext_1',
        timestamp: Date.now(),
        rawText: 'External input',
        injectedText: 'External input',
        mode: 'raw' as const,
      },
    ];
    localStorage.setItem('sona:voice-typing:history', JSON.stringify(externalItems));

    useVoiceTypingHistoryStore.getState().reloadHistory();
    expect(useVoiceTypingHistoryStore.getState().items).toHaveLength(1);
    expect(useVoiceTypingHistoryStore.getState().items[0].rawText).toBe('External input');

    const newerItems = [
      {
        id: 'ext_2',
        timestamp: Date.now(),
        rawText: 'New event input',
        injectedText: 'New event input',
        mode: 'polish' as const,
      },
    ];
    localStorage.setItem('sona:voice-typing:history', JSON.stringify(newerItems));
    window.dispatchEvent(new StorageEvent('storage', { key: 'sona:voice-typing:history' }));

    expect(useVoiceTypingHistoryStore.getState().items).toHaveLength(1);
    expect(useVoiceTypingHistoryStore.getState().items[0].rawText).toBe('New event input');
  });
});
