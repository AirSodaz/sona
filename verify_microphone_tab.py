from playwright.sync_api import sync_playwright

def test_microphone_tab(page):
    # Setup mocks
    page.add_init_script("""
        window.localStorage.setItem('sona-first-run-completed', 'true');
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

    # Click Settings
    settings_btn = page.locator(".header-actions button")
    settings_btn.click()

    # Wait for settings modal
    page.wait_for_selector(".settings-modal")

    # Click Input Device tab
    page.locator("button:has-text('Input Device')").click()

    # Wait for the tab panel
    page.wait_for_selector("#settings-panel-microphone")

    # Take screenshot of the settings modal
    page.locator(".settings-modal").screenshot(path="verification_settings_input.png")

with sync_playwright() as p:
    browser = p.chromium.launch(headless=True)
    page = browser.new_page()
    try:
        test_microphone_tab(page)
    finally:
        browser.close()
