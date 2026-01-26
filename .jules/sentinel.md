## 2025-05-19 - [CRITICAL] Unrestricted Filesystem Scope
**Vulnerability:** The application was configured with `fs:scope: ["**"]`, granting the frontend read/write access to the entire filesystem.
**Learning:** This wildcard scope effectively disables the Tauri sandbox. While it might have been added to simplify file access during development, it poses a severe risk (RCE/LFI) if the frontend is compromised.
**Prevention:** Restrict `fs:scope` to specific application-managed directories (e.g., `$APPDATA/models/**`). For user-selected files, rely on the `dialog` plugin, which automatically grants temporary, scoped permissions for the selected paths without requiring global wildcards.
