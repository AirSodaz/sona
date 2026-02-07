from playwright.sync_api import sync_playwright
import json
import time
import os

def run():
    with sync_playwright() as p:
        browser = p.chromium.launch(headless=True)
        context = browser.new_context()
        page = context.new_page()

        page.on("console", lambda msg: print(f"Console: {msg.text}"))

        # Read the JS file
        with open("verification/mock_tauri.js", "r") as f:
            js_code = f.read()

        page.add_init_script(js_code)

        try:
            page.goto("http://localhost:1420")
            page.wait_for_selector(".app-header")

            # Click History tab
            try:
                page.get_by_role("tab", name="History").click(timeout=2000)
            except:
                page.locator(".tab-button").nth(2).click()

            time.sleep(2)

            items = page.locator(".panel-container div[role='button']")
            count = items.count()
            print(f"Found {count} items in DOM")

            page.screenshot(path="verification/history_final.png")

        except Exception as e:
            print(f"Error: {e}")
        finally:
            browser.close()

if __name__ == "__main__":
    run()
