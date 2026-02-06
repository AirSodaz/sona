from playwright.sync_api import Page, expect, sync_playwright

def verify_refactor(page: Page):
    print("Navigating to app...")
    page.goto("http://localhost:1420")

    # Check if app loaded (header exists)
    expect(page.locator(".app-header")).to_be_visible()
    print("App header visible.")

    # 1. Verify AudioPlayer components
    # AudioPlayer is only visible if audioUrl is set. We can't easily force it without dragging a file.

    # 2. Verify TranscriptEditor components
    # TranscriptEditor is always rendered. Empty state check.
    if page.locator(".transcript-editor").is_visible():
         print("TranscriptEditor list visible.")
    else:
         expect(page.locator(".empty-state")).to_be_visible()
         print("TranscriptEditor empty state visible.")

    # 3. Verify Settings Refactor
    print("Opening settings...")
    # Click settings button. Use generic selector or aria-label if translation is known.
    settings_btn = page.get_by_label("Settings") # Default translation
    if not settings_btn.is_visible():
         # Try locating by icon
         settings_btn = page.locator(".header-actions button").last

    settings_btn.click()

    # Check if modal opens
    expect(page.locator(".settings-modal")).to_be_visible()
    print("Settings modal visible.")

    # Check if Tabs are rendered (SettingsTabButton)
    settings_tabs = page.locator(".settings-modal").get_by_role("tab")
    expect(settings_tabs).to_have_count(3)
    print("Settings tabs verified.")

    # Wait for animation frame/focus
    page.wait_for_timeout(500)

    # Take screenshot
    print("Taking screenshot...")
    page.screenshot(path="/home/jules/verification/refactor_verification.png")

if __name__ == "__main__":
    with sync_playwright() as p:
        browser = p.chromium.launch(headless=True)
        page = browser.new_page()
        try:
            verify_refactor(page)
        except Exception as e:
            print(f"Error: {e}")
            page.screenshot(path="/home/jules/verification/error.png")
            raise e
        finally:
            browser.close()
