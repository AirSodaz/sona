from playwright.sync_api import sync_playwright

def test_live_record(page):
    # Setup mocks
    page.add_init_script("""
        window.localStorage.setItem('sona-first-run-completed', 'true');
        window.localStorage.setItem('sona-config', JSON.stringify({
            state: {
                config: {
                    offlineModelPath: '/mock/path/to/model'
                }
            }
        }));
        window.__TAURI_INTERNALS__ = {
            invoke: async (cmd, args) => {
                if (cmd === 'get_microphone_devices') return [{name: 'Mock Mic 1'}, {name: 'Mock Mic 2'}];
                if (cmd === 'get_system_audio_devices') return [{name: 'Mock System 1'}, {name: 'Mock System 2'}];
                return null;
            }
        };
        window.__TAURI__ = {
            core: window.__TAURI_INTERNALS__,
            event: { listen: async () => (() => {}) }
        };
    """)

    # Go to app
    page.goto("http://localhost:1420")

    # Wait for app to load
    page.wait_for_selector(".app-main")

    # Take screenshot immediately to see empty state visualizer
    page.screenshot(path="verification_live_record_empty.png")

    # Mock some audio data
    page.evaluate("""
        const event = new Event('microphone-audio');
        event.payload = 16000;
        window.dispatchEvent(event);
    """)
    page.wait_for_timeout(500)

    # Take screenshot
    page.screenshot(path="verification_live_record_active.png")

with sync_playwright() as p:
    browser = p.chromium.launch(headless=True)
    page = browser.new_page()
    try:
        test_live_record(page)
    finally:
        browser.close()
