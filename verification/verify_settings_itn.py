from playwright.sync_api import sync_playwright, expect
import time
import os

def run():
    with sync_playwright() as p:
        print("Launching browser...")
        browser = p.chromium.launch(headless=True)
        context = browser.new_context()
        page = context.new_page()

        # Inject Tauri mocks
        page.add_init_script(path="tests/e2e/mocks/tauri.js")

        # 1. Start application
        print("Navigating to app...")
        try:
            page.goto("http://localhost:1420")
        except Exception as e:
            print(f"Failed to load page: {e}")
            return

        # 2. Open Settings
        print("Opening Settings...")
        # Wait for page to be ready
        page.wait_for_timeout(2000)

        try:
            settings_btn = page.get_by_role("button", name="Settings")
            # If not found, try waiting longer or debug
            if settings_btn.count() == 0:
                print("Settings button not found immediately, waiting...")
                page.wait_for_selector('button[aria-label="Settings"]', timeout=5000)

            settings_btn.click()
        except Exception as e:
            print(f"Error finding settings button: {e}")
            page.screenshot(path="/home/jules/verification/error_home.png")
            return

        # 3. Navigate to Local Path (Model Settings)
        print("Navigating to Model Settings...")
        expect(page.get_by_role("dialog")).to_be_visible()

        # 'Model Settings' in English
        tab_btn = page.get_by_role("tab", name="Model Settings")
        tab_btn.click()

        # 4. Verify Enable ITN switch
        print("Verifying Enable ITN switch...")
        page.wait_for_timeout(500)
        # Check text
        expect(page.get_by_text("Enable ITN")).to_be_visible()
        expect(page.get_by_text("Inverse Text Normalization (ITN)")).to_be_visible()

        # 5. Verify ITN Model List
        print("Verifying ITN Model List...")
        expect(page.get_by_text("Chinese Number ITN")).to_be_visible()

        # Scroll to ensure everything is visible
        settings_content = page.locator(".settings-content-scroll")
        settings_content.evaluate("el => el.scrollTop = el.scrollHeight")

        page.wait_for_timeout(500)

        # 6. Take Screenshot
        if not os.path.exists("/home/jules/verification"):
            os.makedirs("/home/jules/verification")

        print("Taking screenshot...")
        page.screenshot(path="/home/jules/verification/verify_settings_itn.png")
        print("Screenshot saved to /home/jules/verification/verify_settings_itn.png")

        browser.close()

if __name__ == "__main__":
    run()
