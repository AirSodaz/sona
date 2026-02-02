import { test, expect } from '@playwright/test';
import { resolve, dirname } from 'path';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

test.describe('Sona App E2E', () => {
  test.beforeEach(async ({ page }) => {
    // Inject Tauri mock
    await page.addInitScript({ path: resolve(__dirname, 'mocks/tauri.js') });

    // Navigate to app
    await page.goto('/');

    // Wait for app to settle
    await page.waitForLoadState('networkidle');
  });

  test('should load application and show initial state', async ({ page }) => {
    await expect(page.locator('body')).toBeVisible();

    // Check for "Start Recording" button
    const recordBtn = page.getByRole('button', { name: /Start Recording/i });
    await expect(recordBtn).toBeVisible();
  });

  test('should simulate recording and transcript generation', async ({ page }) => {
    // 1. Setup initial state via store injection
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

    // 3. Verify Stop Button appears (Wait for it)
    const stopBtn = page.getByRole('button', { name: /Stop/i });
    await expect(stopBtn).toBeVisible();

    // 4. Inject Transcript Segment
    await page.evaluate(() => {
        const { upsertSegment } = window.useTranscriptStore.getState();
        upsertSegment({
            id: 'seg-1',
            start: 0,
            end: 1000,
            text: 'Hello, this is a test transcript.',
            isFinal: true
        });
    });

    // 5. Verify Transcript appears in Editor
    await expect(page.getByText('Hello, this is a test transcript.')).toBeVisible();

    // 6. Stop Recording
    await stopBtn.click();

    // 7. Verify we are back to ready state
    await expect(recordBtn).toBeVisible();
  });

  test('should open and navigate settings', async ({ page }) => {
     await page.evaluate(() => {
        window.useTranscriptStore.getState().setConfig({
            streamingModelPath: '/mock/path/model',
            offlineModelPath: '/mock/path/offline',
        });
    });

    // Click Settings button in sidebar/header (Button with "Settings" text or icon with label)
    // The sidebar usually has a button with text "Settings" or icon
    // Based on en.json: "header.settings": "Settings"
    const settingsBtn = page.getByRole('button', { name: 'Settings' });

    // Note: If multiple buttons have "Settings" (e.g. Warning overlay + Header), force click the header one or first visible
    await settingsBtn.first().click();

    // Verify Modal Open
    const modal = page.getByRole('dialog');
    await expect(modal).toBeVisible();
    await expect(modal).toContainText('Settings');

    // Navigate Tabs (Model Hub)
    await page.getByRole('tab', { name: 'Model Hub' }).click();
    await expect(page.getByRole('tabpanel', { name: 'Model Hub' })).toBeVisible();

    // Close Settings
    await page.getByLabel('Close').click();
    await expect(modal).toBeHidden();
  });

  test('should show export options', async ({ page }) => {
    await page.evaluate(() => {
        const { upsertSegment } = window.useTranscriptStore.getState();
        upsertSegment({ id: '1', start: 0, end: 1, text: 'Export me', isFinal: true });
    });

    // Click Export Button
    const exportBtn = page.getByRole('button', { name: 'Export' });
    await exportBtn.click();

    // Verify Menu
    const menu = page.getByRole('menu');
    await expect(menu).toBeVisible();
    await expect(menu).toContainText('Plain Text');
    await expect(menu).toContainText('JSON');
  });

});
