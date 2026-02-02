from playwright.sync_api import sync_playwright, expect
import os
import json

def test_file_queue_a11y():
    with sync_playwright() as p:
        browser = p.chromium.launch(headless=True)
        page = browser.new_page()

        page.on("console", lambda msg: print(f"CONSOLE: {msg.text}"))

        # 1. Setup LocalStorage & Mock
        page.add_init_script("""
            localStorage.setItem('sona-config', JSON.stringify({
                offlineModelPath: '/mock/model/path',
                streamingModelPath: '/mock/model/path',
                theme: 'light'
            }));
        """)

        # 2. Prepare Tauri Mock with Dialog Support
        mock_path = os.path.join(os.getcwd(), 'tests/e2e/mocks/tauri.js')
        with open(mock_path, 'r') as f:
            mock_script = f.read()

        patch = """
  if (cmd === 'plugin:dialog|open') {
      console.log('[MockTauri] Dialog Open called');
      return ['/path/to/test-file-1.wav', '/path/to/test-file-2.wav'];
  }
"""
        # Ensure we only replace once or check if replaced
        if "Dialog Open called" not in mock_script:
             mock_script_patched = mock_script.replace(
                "window.__TAURI_INTERNALS__.invoke = async (cmd, args) => {",
                "window.__TAURI_INTERNALS__.invoke = async (cmd, args) => {" + patch
            )
        else:
             mock_script_patched = mock_script

        page.add_init_script(mock_script_patched)

        # Navigate
        page.goto("http://localhost:1420")

        # Wait for hydration
        page.wait_for_timeout(500)

        # 3. Go to Batch Import
        print("Clicking Batch Import tab...")
        page.get_by_role("tab", name="Batch Import").click()

        # 4. Trigger File Selection
        print("Clicking Drop Zone...")
        # Ensure drop zone is visible
        expect(page.locator(".drop-zone")).to_be_visible()
        page.locator(".drop-zone").click()

        # 5. Verify Queue List Semantics
        print("Waiting for Queue...")
        queue_list = page.get_by_role("list", name="Queue (2)")
        expect(queue_list).to_be_visible(timeout=10000)

        # Expect list items
        items = queue_list.get_by_role("listitem")
        expect(items).to_have_count(2)

        # 6. Verify Delete Button is Focusable
        print("Verifying Delete Button...")
        delete_btn = items.nth(0).get_by_label("Delete")

        expect(delete_btn).not_to_have_attribute("tabindex", "-1")

        delete_btn.focus()

        # Check visibility with robust assertion (waits for transition)
        expect(delete_btn).to_have_css("opacity", "1")

        print("Verification: List semantics correct, delete button focusable and visible on focus.")

        # Screenshot
        page.screenshot(path="verification/file_queue_verification.png")

        browser.close()

if __name__ == "__main__":
    test_file_queue_a11y()
