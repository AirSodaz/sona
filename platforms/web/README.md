# Sona Web Client (Standalone)

This directory contains the standalone Web client (Remote Web Editor) decoupled from `platforms/desktop`.

## Architecture & Background
The Web client was originally bundled inside the desktop client's frontend and served by the desktop HTTP API server adapter. To eliminate browser origin/credential isolation issues (`401 Unauthorized` under separate origins) and maintain a clean separation of concerns, the Web client is decoupled from the desktop platform.

## Contents
- `src/components/RemoteWebEditor.tsx`: Browser-based transcription and subtitle editing client.
- `src/services/apiServerClient.ts`: SDK client for connecting to the Sona API server.
- `src/utils/webExport.ts`: Multi-format subtitle/transcript exporter (SRT, VTT, TXT, Markdown).
- `src/styles/remote-web-editor.css`: Styles for the Remote Web Editor.
- `tests/`: Unit tests for the web client components, API client, and export utilities.
