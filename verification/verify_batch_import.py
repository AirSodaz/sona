from playwright.sync_api import sync_playwright, expect
import os

def test_batch_import_ui():
    with sync_playwright() as p:
        browser = p.chromium.launch(headless=True)
        page = browser.new_page()

        # Inject Tauri mock
        mock_path = os.path.join(os.getcwd(), 'tests/e2e/mocks/tauri.js')
        with open(mock_path, 'r') as f:
            mock_script = f.read()
        page.add_init_script(mock_script)

        # Navigate
        page.goto("http://localhost:1420")

        # Click on Batch Import tab
        # The tab has text "Batch Import" and a Folder icon
        batch_tab = page.get_by_role("tab", name="Batch Import")
        batch_tab.click()

        # Wait for the drop zone to appear
        drop_zone = page.locator(".drop-zone")
        expect(drop_zone).to_be_visible()

        # Find the "Select File" element
        select_file_btn = page.locator(".drop-zone .btn-primary")

        # Verify text
        expect(select_file_btn).to_have_text("Select File")

        # Verify it is NOT a button role (it should be hidden from accessibility tree or just a generic element)
        # If I look for a button with name "Select File", it should NOT be there because of aria-hidden="true"
        # OR because it's a div without role="button".

        # This expect should FAIL if it WAS a button. So we expect count to be 0.
        expect(page.get_by_role("button", name="Select File")).to_have_count(0)

        print("Verification: 'Select File' is not recognized as a button by screen readers (Good!)")

        # Take screenshot
        page.screenshot(path="verification/batch_import_verification.png")
        print("Screenshot saved to verification/batch_import_verification.png")

        browser.close()

if __name__ == "__main__":
    test_batch_import_ui()
