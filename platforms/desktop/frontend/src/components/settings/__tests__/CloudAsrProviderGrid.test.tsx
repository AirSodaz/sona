import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { setTestConfig } from '../../../test-utils/configTestUtils';
import { CloudAsrProviderGrid } from '../CloudAsrProviderGrid';

const mockTestOnlineAsrProvider = vi.fn();
const mockOpenUrl = vi.fn();

vi.mock('../../../services/tauri/recognizer', () => ({
  testOnlineAsrProvider: (...args: unknown[]) => mockTestOnlineAsrProvider(...args),
}));

vi.mock('../../../services/tauri/platform/opener', () => ({
  openUrl: (...args: unknown[]) => mockOpenUrl(...args),
}));
vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, options?: { defaultValue?: string } & Record<string, unknown>) => {
      if (typeof options?.defaultValue === 'string') {
        return options.defaultValue.replace(/\{\{(\w+)\}\}/g, (_: string, variable: string) =>
          String(options?.[variable] ?? '')
        );
      }
      return key;
    },
  }),
  initReactI18next: {
    type: '3rdParty',
    init: () => undefined,
  },
}));

describe('CloudAsrProviderGrid', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    setTestConfig({
      asr: {
        selections: {
          live: { engine: 'local', mode: 'streaming', modelPath: '/path' },
          batch: { engine: 'local', mode: 'batch', modelPath: '/path' },
          caption: { engine: 'local', mode: 'streaming', modelPath: '/path' },
          voiceTyping: { engine: 'local', mode: 'streaming', modelPath: '/path' },
        },
        providers: {
          online: {
            'volcengine-doubao': {
              apiKey: 'volc-test-key',
              batchEndpoint: 'https://openspeech.bytedance.com/api/v3/auc/bigmodel/recognize/flash',
              batchResourceId: 'volc.bigasr.auc_turbo',
            },
          },
        },
      },
    });
  });

  it('renders provider cards with model tags and status chip, without active-status clutter', () => {
    render(<CloudAsrProviderGrid />);

    // 1. Shows Volcengine and other providers
    expect(screen.getByText('Volcengine')).toBeDefined();
    expect(screen.getByText('Groq')).toBeDefined();
    expect(screen.getByText('Deepgram')).toBeDefined();

    // 2. Volcengine should show "Added" status chip since batch flash config is present with default models
    const addedChips = screen.getAllByText('Added');
    expect(addedChips.length).toBeGreaterThanOrEqual(1);

    // 3. Unconfigured providers show "Not Configured"
    const unconfiguredChips = screen.getAllByText('Not Configured');
    expect(unconfiguredChips.length).toBeGreaterThanOrEqual(1);

    // 4. Verifies no noisy "Active" indicators are rendered
    expect(screen.queryByText(/Active/i)).toBeNull();
    // 5. Verifies model tags exist (Live, Batch, Cloud)
    const liveTags = screen.getAllByText('Live');
    expect(liveTags.length).toBeGreaterThanOrEqual(1);
    const batchTags = screen.getAllByText('Batch');
    expect(batchTags.length).toBeGreaterThanOrEqual(1);
    const cloudTags = screen.getAllByText('Cloud');
    expect(cloudTags.length).toBeGreaterThanOrEqual(1);
    // 6. Verifies models are folded by default (not visible in DOM)
    expect(screen.queryByText('Seed-ASR 极速版 (Flash)')).toBeNull();
    expect(screen.queryByText('Whisper Large v3 Turbo')).toBeNull();
  });

  it('expands to reveal api key input, console link and models without spec sheet clutter', async () => {
    render(<CloudAsrProviderGrid />);

    // Expand Deepgram card
    const deepgramHeader = screen.getByRole('button', { name: /Deepgram/ });
    fireEvent.click(deepgramHeader);

    // Verify console link is available and clicking it opens the URL
    const consoleLinks = screen.getAllByRole('link', { name: /Get API Key/i });
    expect(consoleLinks.length).toBeGreaterThanOrEqual(1);
    expect(consoleLinks[0].getAttribute('href')).toBe('https://console.deepgram.com/');

    fireEvent.click(consoleLinks[0]);
    expect(mockOpenUrl).toHaveBeenCalledWith('https://console.deepgram.com/');

    // Verify eye button toggles password visibility
    const apiKeyInput = screen.getByPlaceholderText('API Key');
    expect(apiKeyInput.getAttribute('type')).toBe('password');
    const eyeBtn = screen.getByRole('button', { name: /show password/i });
    fireEvent.click(eyeBtn);
    expect(apiKeyInput.getAttribute('type')).toBe('text');
    fireEvent.click(eyeBtn);
    expect(apiKeyInput.getAttribute('type')).toBe('password');

    // Verify spec sheet cards are removed
    expect(screen.queryByText('Core Model')).toBeNull();
    expect(screen.queryByText('Strengths')).toBeNull();
    expect(screen.queryByText('Specifications & Limits')).toBeNull();
    expect(screen.queryByText('Best For')).toBeNull();

    // Verify models are now visible
    expect(screen.getByText('Nova-3')).toBeDefined();
    expect(screen.getByText('Nova-2')).toBeDefined();
  });
  it('triggers model-level connectivity test from icon-only button and shows feedback', async () => {
    mockTestOnlineAsrProvider.mockResolvedValueOnce(128);

    render(<CloudAsrProviderGrid />);

    // Expand Volcengine card first (models fold with config)
    const volcengineHeader = screen.getByRole('button', { name: /Volcengine/i });
    fireEvent.click(volcengineHeader);

    // Find the icon-only test button for Seed-ASR
    const testModelBtn = screen.getByRole('button', {
      name: /Test Connection Seed-ASR.*Flash/i,
    });

    // Check that individual model does not render description
    expect(screen.queryByText('极速同步直回，秒级出字，适合批量音频文件导入')).toBeNull();

    // Check that common tags (Cloud, Batch) are not displayed on model row
    const flashRow = testModelBtn.closest('.model-version-row');
    expect(flashRow).toBeDefined();
    // It should have differentiating tags: Flash, Turbo, 2.0
    expect(within(flashRow as HTMLElement).getByText('Flash')).toBeDefined();
    expect(within(flashRow as HTMLElement).getByText('Turbo')).toBeDefined();
    // It should NOT have common tags: Cloud, Batch
    expect(within(flashRow as HTMLElement).queryByText('Cloud')).toBeNull();
    expect(within(flashRow as HTMLElement).queryByText('Batch')).toBeNull();

    fireEvent.click(testModelBtn);

    expect(mockTestOnlineAsrProvider).toHaveBeenCalledWith(
      'volcengine-doubao',
      expect.objectContaining({
        apiKey: 'volc-test-key',
        model: 'volc.bigasr.auc_turbo',
      })
    );
  });

  it('supports adding and deleting models per provider and disables add without api key', async () => {
    render(<CloudAsrProviderGrid />);

    // Expand Deepgram card (no API key configured)
    const deepgramHeader = screen.getByRole('button', { name: /Deepgram/ });
    fireEvent.click(deepgramHeader);

    // Deepgram currently has no API key, so Add button is disabled
    const addNova3Btn = screen.getByRole('button', { name: /Add Nova-3/i });
    expect(addNova3Btn).toBeDefined();
    expect(addNova3Btn.hasAttribute('disabled')).toBe(true);
    expect(addNova3Btn.getAttribute('data-tooltip')).toBe('Configure API Key first');

    // Clicking disabled Add button does nothing
    fireEvent.click(addNova3Btn);
    expect(screen.queryByRole('button', { name: /Delete Nova-3/i })).toBeNull();

    // Expand Volcengine card (has API key configured)
    const volcengineHeader = screen.getByRole('button', { name: /Volcengine/i });
    fireEvent.click(volcengineHeader);

    // Volcengine has Flash added by default, so it shows "Delete" button
    const deleteFlashBtn = screen.getByRole('button', { name: /Delete Seed-ASR.*Flash/i });
    expect(deleteFlashBtn).toBeDefined();

    fireEvent.click(deleteFlashBtn);

    // After clicking Delete Flash, it should now show enabled Add button
    const addFlashBtn = screen.getByRole('button', { name: /Add Seed-ASR.*Flash/i });
    expect(addFlashBtn).toBeDefined();
    expect(addFlashBtn.hasAttribute('disabled')).toBe(false);

    // Click Add Flash to re-add it
    fireEvent.click(addFlashBtn);
    expect(screen.getByRole('button', { name: /Delete Seed-ASR.*Flash/i })).toBeDefined();
  });
  it('displays error feedback on model test button tooltip when connectivity test fails', async () => {
    mockTestOnlineAsrProvider.mockRejectedValueOnce(new Error('Invalid API Key (HTTP 401)'));

    render(<CloudAsrProviderGrid />);

    // Expand Volcengine card
    const volcengineHeader = screen.getByRole('button', { name: /Volcengine/i });
    fireEvent.click(volcengineHeader);

    const testModelBtn = screen.getByRole('button', {
      name: /Test Connection Seed-ASR.*Flash/i,
    });
    fireEvent.click(testModelBtn);

    await waitFor(() => {
      expect(testModelBtn.getAttribute('data-tooltip')).toContain('Invalid API Key (HTTP 401)');
    });
  });
});
