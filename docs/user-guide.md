# Sona User Guide

[English](user-guide.md) | [简体中文](user-guide.zh-CN.md) | [Project README](../README.md)

This guide is for desktop users who want to install Sona, complete the first setup, transcribe audio, edit transcripts, and export results.

The visuals in this repository are reference illustrations. They use stable file paths so they can be replaced later with live screenshots without changing the guide structure.

## 1. What Sona Is For

Sona is a privacy-first transcript editor for people who want local speech-to-text without sending audio to a cloud service by default.

Sona is a good fit if you want to:

- record meetings, lectures, interviews, or notes with `Live Record`
- transcribe existing audio or video files with `Batch Import`
- edit timestamps and text in a transcript-focused editor
- optionally polish or translate text with an AI provider you configure yourself
- export subtitles or plain text in common formats

## 2. Install And Launch

For most users, install Sona from the latest release build:

- Download the app from [GitHub Releases](https://github.com/AirSodaz/sona/releases/latest).
- Launch Sona.
- If you are building from source instead, use the setup steps in the [project README](../README.md).

When Sona opens for the first time, it may block the main workflow with `First Run Setup`. If setup was postponed, Sona can also show a reminder banner until the required offline model setup is complete.

## 3. First Run Setup

Before you can transcribe audio locally, Sona needs a working offline model setup.

### Preconditions

- Sona is installed and can open normally.
- You have an internet connection if you want Sona to download the recommended model pack during onboarding.

### Steps

1. Launch Sona and wait for `First Run Setup`.
2. Review the welcome step. Sona's recommended path is `Microphone -> Live Record`.
3. Continue to the models step and click `Download Recommended Models`.
4. Let Sona finish downloading the recommended offline model pack.
5. Continue to the microphone step and allow microphone access.
6. Choose the microphone you want Sona to use for `Live Record`.
7. Click `Start with Live Record`.

### Result

- Sona applies the recommended offline setup for local transcription.
- The app opens on `Live Record`.
- If setup is not finished, the reminder banner can reopen onboarding later.

### Notes

- The recommended first-run setup is focused on getting local live transcription working quickly.
- You can change models later in `Settings > Model Hub`.
- You can change the default input device later in `Settings > Input Device`.

## 4. Live Record

Use `Live Record` when you want to capture speech in real time and see the transcript update as audio is processed.

### Preconditions

- You completed `First Run Setup`, or you manually configured a `Live Record Model`.
- Your microphone permission is granted if you want to record from a microphone.

### Steps

1. Click the `Live Record` tab in the top navigation.
2. If you are not already recording, choose an input source from the dropdown:
   `Microphone` or `Desktop Audio`.
3. Click the red `Start Recording` button.
4. Watch the waveform and timer while Sona captures audio.
5. Use `Pause` or `Stop` when needed.
6. Open `Parameter Settings` if you want to adjust:
   `Subtitle Mode`, `Language`, `Auto-Polish`, or `Auto-Polish Frequency`.
7. If you use live captions, enable `Live Caption` and adjust floating window behavior in `Settings > Subtitle Settings`.

### Result

- Transcript segments appear in the editor on the right.
- The currently active segment can follow playback and recording state.
- If `Auto-Polish` is enabled and AI is configured, Sona can polish final segments in batches.

### Notes

- `Ctrl + Space` starts or stops live recording.
- `Space` pauses or resumes while recording is active.
- If Sona says a model is missing, reopen onboarding or configure models in `Settings > Model Hub`.

## 5. Batch Import

Use `Batch Import` when you already have audio or video files and want Sona to transcribe them in the background.

### Preconditions

- You configured a `Batch Import Model`.
- Your input file uses a supported audio or video format.

### Steps

1. Click the `Batch Import` tab.
2. Drop files into the import area, or click `Select File`.
3. Add one or more files.
4. Watch the queue sidebar and active item progress view.
5. Use `Add More Files` if you want to keep building the queue.
6. Open `Parameter Settings` to adjust `Subtitle Mode`, `Language`, and optional `Auto-Polish` behavior before or during processing.
7. When a file finishes, review the transcript in the editor.

### Result

- Sona processes files into a queue with `Pending`, `Processing`, `Complete`, or `Failed` states.
- Completed items load into the main transcript editor for editing, translation, and export.

### Notes

- If no offline batch model is configured, Sona reopens onboarding instead of starting import.
- `Settings > Model Settings` includes `Max Concurrent Transcriptions` and `VAD Buffer Size`, which affect batch behavior.

## 6. Transcript Editing And Playback

After Sona creates transcript segments, the editor becomes the main place to review and refine them.

### Preconditions

- You already created transcript content with `Live Record`, `Batch Import`, or `History`.

### Steps

1. Review the segment list in the editor panel.
2. Click a timestamp to seek playback to that time.
3. Double-click segment text, or click the edit action, to start editing.
4. Press `Enter` to save the current segment.
5. Press `Shift + Enter` to insert a line break while editing.
6. Use the merge action to combine a segment with the next one.
7. Use the delete action to remove a segment after confirmation.
8. Press `Ctrl + F` to search inside the transcript.
9. Use the audio player to play, pause, seek, change speed, and control volume when an audio file is available.

### Result

- The transcript stays editable at the segment level.
- Playback and transcript navigation stay aligned through timestamps.

### Notes

- The editor toolbar only appears while a segment is actively being edited.
- The toolbar supports `Undo`, `Redo`, `Bold`, `Italic`, `Underline`, and line breaks.
- Search can jump between matching segments without leaving the editor.

## 7. AI Polish And Translation

Sona's AI features are optional. Local transcription works without them, but `AI Polish` and `Translate` require an AI provider configuration.

### Preconditions

- You already have transcript segments.
- In `Settings > AI Service`, you configured:
  `AI Service Type`, `Base URL`, `API Key`, `Model Name`, and optionally `Temperature`.

### Steps For AI Polish

1. Open `Settings > AI Service`.
2. Choose a provider such as `OpenAI`, `Anthropic`, `Ollama`, `Google Gemini`, `DeepSeek`, `Kimi`, or `SiliconFlow`.
3. Enter the provider connection details.
4. Use `Test Connection` to confirm the configuration.
5. Return to the main workspace and click the `AI Polish` button.
6. Use the menu to start polishing, re-transcribe, undo polish, or redo polish depending on current state.

### Steps For Translate

1. Make sure the transcript is loaded in the editor.
2. Click the `Translate` button.
3. Pick a target language from the dropdown menu.
4. Click `Start Translation` or `Retranslate`.
5. Use `Show Translations` or `Hide Translations` to control bilingual display in the editor.

### Result

- `AI Polish` updates transcript text in place.
- `Translate` stores translation text per segment and can display it under the original text.

### Notes

- Translation target languages currently include `Chinese (Simplified)`, `English`, `Japanese`, `Korean`, `French`, `German`, and `Spanish`.
- `Auto-Polish` in `Parameter Settings` depends on a valid AI configuration.

## 8. Export Subtitles And Text

Export is available when the transcript contains at least one segment.

### Preconditions

- You have transcript content in the editor.

### Steps

1. Click the `Export` button in the header.
2. Choose an export mode:
   `Original`, `Translation`, or `Bilingual`.
3. Pick an output format:
   `SubRip (.srt)`, `WebVTT (.vtt)`, `JSON (.json)`, or `Plain Text (.txt)`.
4. Save the exported file.

### Result

- Sona writes the transcript to the selected format.
- If translations exist, you can export translated-only or bilingual output.

### Notes

- `Translation` and `Bilingual` modes are only available when at least one segment contains translation text.
- `Original` mode is always available.

## 9. History And Common Settings

Use `History` to reopen earlier work, and use `Settings` to manage the app's defaults.

### History Steps

1. Click the `History` tab.
2. Search by title or transcript content.
3. Filter by type with `All Types`, `Recordings`, or `Batch Imports`.
4. Filter by time with `Any Time`, `Today`, `Last 7 Days`, or `Last 30 Days`.
5. Click an item to load it.
6. Use selection mode if you want to delete multiple items.

### Common Settings To Review

- `Settings > General`
  theme, app language, font, tray behavior, automatic update checks
- `Settings > Input Device`
  microphone selection, system audio selection, microphone boost, mute during recording
- `Settings > Subtitle Settings`
  live caption startup, click-through window lock, always-on-top, font size, width, color
- `Settings > Model Hub`
  `Live Record Model`, `Batch Import Model`, recognition models, punctuation models, VAD models
- `Settings > Model Settings`
  `VAD Buffer Size`, `Max Concurrent Transcriptions`, ITN settings, restore defaults
- `Settings > Shortcuts`
  playback, live recording, search, and editor shortcuts
- `Settings > About`
  source code link and update check

## 10. FAQ And Troubleshooting

### Sona keeps asking me to finish setup

- Open the onboarding banner and complete the model and microphone steps.
- If you skipped setup earlier, make sure `Live Record Model` and `Batch Import Model` are both configured in `Settings > Model Hub`.

### `Live Record` does not start

- Check microphone permission in your operating system.
- Confirm a model is configured for live transcription.
- Try switching the input source between `Microphone` and `Desktop Audio` if the wrong capture source is selected.

### `Batch Import` does not start

- Make sure `Batch Import Model` is configured.
- Confirm the file extension is supported.
- If Sona reports an unsupported format, convert the file to a supported audio or video format before importing again.

### `AI Polish` or `Translate` is disabled or fails

- Confirm `Settings > AI Service` has a valid provider, base URL, API key, and model.
- Use `Test Connection` before retrying.
- If your provider uses a custom base URL or a local service such as `Ollama`, verify that endpoint first.

### Export only shows `Original`

- `Translation` and `Bilingual` export modes only appear when the transcript already contains translation text.

### Playback controls are missing

- The audio player only appears when the current transcript has an audio URL available, such as a loaded history item or processed file.

### I want to build or develop Sona

- Use the [project README](../README.md) for source build and development commands.
