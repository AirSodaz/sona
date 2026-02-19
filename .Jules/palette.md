## 2026-02-19 - Search Empty State Accessibility
**Learning:** Empty search states like "0/0" are technically correct but user-hostile. Explicit "No results" text combined with `aria-invalid` and color cues (red border) provides immediate, unambiguous feedback for all users.
**Action:** Always audit empty states in search/filter components for clarity and accessibility. Use `aria-invalid` on inputs when they produce no matches.
