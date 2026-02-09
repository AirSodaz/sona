import { test, expect } from '@playwright/test';
import { resolve, dirname } from 'path';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

test.describe('Mock Stream E2E', () => {
  test.beforeEach(async ({ page }) => {
    // Inject Tauri mock
    await page.addInitScript({ path: resolve(__dirname, 'mocks/tauri.js') });

    // Navigate to app
    await page.goto('/');

    // Wait for app to settle
    await page.waitForLoadState('networkidle');
  });

  test('should receive segments from mock stream', async ({ page }) => {
    // 1. Setup config to ensure recording can start
    await page.evaluate(() => {
        window.useTranscriptStore.getState().setConfig({
            streamingModelPath: '/mock/path/model',
            offlineModelPath: '/mock/path/offline',
            enableITN: false
        });
    });

    // 2. Start Recording
    const recordBtn = page.getByRole('button', { name: /Start Recording/i });
    await recordBtn.click();

    // 3. Verify Stop Button appears (indicating recording started)
    const stopBtn = page.getByRole('button', { name: /Stop/i });
    await expect(stopBtn).toBeVisible();

    // 4. Wait for mock segment to appear
    // The mock stream emits every 2000ms.
    // "Mock transcript segment 1"

    // We increase timeout to ensure we catch the first segment (2s delay)
    await expect(page.getByText('Mock transcript segment 1')).toBeVisible({ timeout: 10000 });

    // 5. Verify a second segment to ensure stream continues
    await expect(page.getByText('Mock transcript segment 2')).toBeVisible({ timeout: 5000 });

    // 6. Stop Recording
    await stopBtn.click();
  });
});
