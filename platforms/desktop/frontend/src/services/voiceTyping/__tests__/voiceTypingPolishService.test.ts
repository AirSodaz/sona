import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useConfigStore } from '../../../stores/configStore';
import { completeLlm } from '../../tauri/llm';
import {
  cleanPolishedOutput,
  DEFAULT_VOICE_TYPING_POLISH_PROMPT,
  polishVoiceTypingText,
} from '../voiceTypingPolishService';

vi.mock('../../tauri/llm', () => ({
  completeLlm: vi.fn(),
}));

describe('voiceTypingPolishService', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useConfigStore.setState({
      config: {
        ...useConfigStore.getState().config,
        llmSettings: {
          activeProvider: 'ollama',
          providers: {
            ollama: {
              baseUrl: 'http://localhost:11434',
              apiKey: '',
              model: 'qwen2.5:3b',
            },
          },
          selections: {
            polishModel: { provider: 'ollama', model: 'qwen2.5:3b' },
          },
        } as any,
      },
    });
  });

  it('cleans markdown code block and surrounding quotes', () => {
    expect(cleanPolishedOutput('```\nHello world\n```')).toBe('Hello world');
    expect(cleanPolishedOutput('```markdown\nHello world\n```')).toBe('Hello world');
    expect(cleanPolishedOutput('"Hello world"')).toBe('Hello world');
    expect(cleanPolishedOutput('“你好世界”')).toBe('你好世界');
  });

  it('returns empty text directly if input is whitespace', async () => {
    const result = await polishVoiceTypingText('   ');
    expect(result).toBe('');
    expect(completeLlm).not.toHaveBeenCalled();
  });

  it('calls completeLlm with default prompt and cleans output', async () => {
    vi.mocked(completeLlm).mockResolvedValueOnce({
      text: '今天下午两点在三号会议室开会。',
      model: 'qwen2.5:3b',
      provider: 'ollama',
    } as any);

    const raw = '就是说今天下午2点在那个3号会议室开会';
    const result = await polishVoiceTypingText(raw);

    expect(completeLlm).toHaveBeenCalledTimes(1);
    expect(vi.mocked(completeLlm).mock.calls[0][0].systemPrompt).toBe(
      DEFAULT_VOICE_TYPING_POLISH_PROMPT
    );
    expect(vi.mocked(completeLlm).mock.calls[0][0].input).toBe(raw);
    expect(result).toBe('今天下午两点在三号会议室开会。');
  });

  it('falls back to raw text and calls onError if completeLlm fails', async () => {
    const error = new Error('Network error');
    vi.mocked(completeLlm).mockRejectedValueOnce(error);
    const onError = vi.fn();

    const raw = '原始输入内容';
    const result = await polishVoiceTypingText(raw, { onError });

    expect(result).toBe(raw);
    expect(onError).toHaveBeenCalledWith(error);
  });

  it('uses custom prompt if configured', async () => {
    useConfigStore.setState({
      config: {
        ...useConfigStore.getState().config,
        voiceTypingPolishPrompt: '只翻译为英文',
      },
    });

    vi.mocked(completeLlm).mockResolvedValueOnce({
      text: 'Hello World',
    } as any);

    const result = await polishVoiceTypingText('你好世界');

    expect(vi.mocked(completeLlm).mock.calls[0][0].systemPrompt).toBe('只翻译为英文');
    expect(result).toBe('Hello World');
  });

  it('transforms selected text with instruction using completeLlm', async () => {
    const { transformSelectedText } = await import('../voiceTypingPolishService');
    vi.mocked(completeLlm).mockResolvedValueOnce({
      text: 'Translated text',
    } as any);

    const result = await transformSelectedText('选中的原文', '翻译成英文');
    expect(result).toBe('Translated text');
  });

  it('falls back to instruction and calls onError if transformSelectedText fails', async () => {
    const { transformSelectedText } = await import('../voiceTypingPolishService');
    const error = new Error('Transform timeout');
    vi.mocked(completeLlm).mockRejectedValueOnce(error);
    const onError = vi.fn();

    const result = await transformSelectedText('选中的原文', '翻译成英文', { onError });
    expect(result).toBe('翻译成英文');
    expect(onError).toHaveBeenCalledWith(error);
  });
});
