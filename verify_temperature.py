import time
from playwright.sync_api import sync_playwright

def run(playwright):
    browser = playwright.chromium.launch(headless=True)
    context = browser.new_context()
    page = context.new_page()

    # Disable sonaconfig modal check
    page.goto("http://localhost:3000/")
    page.evaluate("localStorage.setItem('sona-first-run-completed', 'true')")
    page.goto("http://localhost:3000/")

    # Wait for the main page to load
    page.wait_for_selector('button[aria-label="Settings"]', state="visible", timeout=10000)

    # Click Settings
    page.locator('button[aria-label="Settings"]').click(force=True)

    # Click AI Service Tab
    page.wait_for_selector('button:has-text("AI Service")', state="visible", timeout=5000)
    page.locator('button:has-text("AI Service")').click(force=True)

    # Wait for the AI Service tab contents to be visible
    page.wait_for_selector('label:has-text("Temperature")', state="visible", timeout=5000)

    # Give it a tiny bit of time to settle visually
    time.sleep(1)

    # Capture screenshot of the settings modal
    page.locator('.settings-content').screenshot(path="/tmp/verification.png")

    context.close()
    browser.close()

with sync_playwright() as playwright:
    run(playwright)
