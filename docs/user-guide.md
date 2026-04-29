# Sona User Guide

[English](user-guide.md) | [简体中文](user-guide.zh-CN.md) | [Project README](../README.md)

This guide is for desktop users who want to install Sona, finish the first setup, transcribe audio locally, refine transcripts, and export results.

## 1. What Sona Is For

Sona is a privacy-first transcript editor for people who want speech-to-text workflows that stay on their own device by default.

Sona is a good fit if you want to:

- capture meetings, lectures, interviews, or notes with `Live Record`
- transcribe existing audio or video files with `Batch Import`
- organize saved recordings and imports with `Workspace`, `Projects`, and `Inbox`
- review timestamps and edit transcripts segment by segment
- optionally use `LLM Polish` or `Translate` after configuring your own provider
- export subtitles or plain text in common formats
- optionally use `Voice Typing` to dictate text into other applications

If you mainly came for `Live Caption`, jump to `Live Record`. If you mainly want to organize saved work, jump to `Workspace, Projects, And Inbox`. If you mainly want `Voice Typing`, jump to `Export And Settings`, especially the `Settings > Voice Typing` section.

## 2. Install And Launch

For most users, the simplest path is the latest release build:

- Download Sona from [GitHub Releases](https://github.com/AirSodaz/sona/releases/latest).
- Launch the app.
- If you are building from source, use the setup instructions in the [project README](../README.md).

When Sona opens for the first time, it can block the main workflow with `First Run Setup`. If you defer setup, Sona can later show a reminder banner until the required offline setup is complete.

### Need CLI?

If you want command-line batch transcription, read the dedicated [CLI guide](cli.md). This user guide stays focused on the desktop app.

## 3. First Run Setup

Before Sona can transcribe audio locally, it needs a working offline model setup.

### Preconditions

- Sona can open normally.
- You have an internet connection if you want Sona to download the recommended model pack during onboarding.

### Steps

1. Launch Sona and wait for `First Run Setup`.
2. Review the welcome step. Sona's recommended first success path is `Microphone -> Live Record`.
3. Click `Continue`.
4. On the models step, click `Download Recommended Models`. If the required models are already installed, Sona can show `Continue` instead.
5. Wait for the recommended offline models to finish downloading and extracting.
6. Continue to the microphone step and allow microphone access.
7. If microphone permission was denied, use `Try Permission Again` after fixing the OS permission prompt.
8. Choose the microphone you want Sona to use for `Live Record`.
9. Click `Start with Live Record`.

### Result

- Sona applies the recommended offline setup for local transcription.
- The app opens on `Live Record`.
- If setup is still incomplete, the reminder banner can reopen onboarding later.

### Notes

- The onboarding flow is designed to get local transcription working quickly with the recommended offline model pack.
- You can click `Later` during onboarding and return from the reminder banner.
- If you hide the reminder banner, it will stop appearing on the home screen until setup is complete.
- You can change models later in `Settings > Model Settings`.
- You can change the default microphone later in `Settings > Input Device`.

## 4. Live Record

Use `Live Record` when you want to capture speech in real time and see transcript segments appear as audio is processed.

### Preconditions

- You completed `First Run Setup`, or you manually configured a `Live Record Model` in `Settings > Model Settings`.
- Your operating system has granted microphone permission if you want to record from a microphone.

### Steps

1. Click the `Live Record` tab.
2. Before recording starts, choose an input source from the dropdown:
   `Microphone` or `Desktop Audio`.
3. Click `Start Recording`.
4. Watch the waveform and timer while Sona captures audio.
5. Use `Pause` to temporarily stop the live session without finishing it, or `Stop` to finalize the recording.
6. Click `Parameter Settings` if you want to adjust `Subtitle Mode` or `Language`.
7. Turn on `Live Caption` if you want the floating caption window during live use.
8. Open `Settings > Subtitle Settings` if you want to change caption behavior such as always-on-top, click-through, size, width, color, or startup behavior.

### What `Live Caption` is for

- `Live Caption` is the `System Audio Captions` toggle on the `Live Record` page, and it is useful when you mainly want a floating subtitle window for system audio.
- You can turn it on without starting a recording first. If you later start `Live Record`, both can run in parallel.
- `Settings > Subtitle Settings` controls caption startup behavior, always-on-top, click-through, font size, width, color, and background transparency.

### Result

- Transcript segments appear in the editor on the right.
- The active segment follows the live recording state.
- While recording is still active, Sona can already surface the session as a `Draft` item in `Workspace`.
- When recording stops, Sona keeps the finished transcript available for editing, polishing, translation, export, and later reopening from `Workspace`, and it completes that same draft item instead of creating a second saved copy.
- If you were not already inside a project, the saved item usually lands in `Inbox`.

### Notes

- `Ctrl + Space` starts or stops live recording by default.
- `Space` pauses or resumes while recording is active.
- `Parameter Settings` covers transcription behavior like `Subtitle Mode` and `Language`, not the full LLM polish workflow.
- If Sona says a model is missing, reopen onboarding or configure models in `Settings > Model Settings`.

## 5. Batch Import

Use `Batch Import` when you already have audio or video files and want Sona to transcribe them in the background.

### Preconditions

- You configured a `Batch Import Model` in `Settings > Model Settings`.
- Your file uses a supported audio or video format.

### Steps

1. Click the `Batch Import` tab.
2. Drop files into the import area, or click `Select File`.
3. Add one or more files to the queue.
4. Watch the queue sidebar and the active item status view.
5. Use `Add More Files` if you want to keep building the queue.
6. Click `Parameter Settings` if you want to adjust `Subtitle Mode` or `Language` for new work.
7. When a file finishes, review the transcript in the main editor.

### Result

- Sona processes files in a queue with `Pending`, `Processing`, `Complete`, or `Failed` states.
- Completed items load into the main transcript editor for editing, translation, and export.

### Notes

- If no offline batch model is configured, Sona reopens onboarding instead of starting import.
- `Settings > Model Settings` includes `VAD Buffer Size` and `Max Concurrent Transcriptions`, which affect batch behavior.

## 6. Transcript Editing And Playback

After Sona creates transcript segments, the editor becomes the main place to review and refine them.

### Preconditions

- You already created transcript content with `Live Record`, `Batch Import`, or `Workspace`.

### Steps

1. Review the segment list in the editor.
2. Click a timestamp to seek playback to that time.
3. Double-click segment text, or use the edit action, to start editing.
4. Press `Enter` to save the current segment.
5. Press `Shift + Enter` to insert a line break while editing.
6. Use the merge action to combine a segment with the next one.
7. Use the delete action to remove a segment after confirmation.
8. Press `Ctrl + F` to search inside the transcript.
9. Use the audio player to play, pause, seek, change speed, or control volume when an audio file is available.

### Result

- The transcript stays editable at the segment level.
- Playback and transcript navigation remain aligned through timestamps.

### Notes

- The editor toolbar only appears while a segment is actively being edited.
- The toolbar supports `Undo`, `Redo`, `Bold`, `Italic`, `Underline`, and line breaks.
- Search can jump between matching segments without leaving the editor.

## 7. LLM Polish, Translation, And Summary

Sona's LLM features are optional. Local transcription works without them, but `LLM Polish`, `Translate`, and `AI Summary` require setup in `Settings > LLM Service`.

### Preconditions

- You already have transcript segments.
- You configured the feature you want to use in `Settings > LLM Service`.

### LLM Service Setup

1. Open `Settings > LLM Service`.
2. In `Feature Models`, choose the model for `Polish Model`, `Translation Model`, and `Summary Model`.
3. In `Provider Credentials`, open the provider you want to use and fill in its connection details such as `Base URL`, `API Key`, `Endpoint`, `Deployment Name`, or provider-specific fields.
4. Click `Test Connection` after entering credentials.
5. Return to the main workspace after the required feature model is assigned.

### Steps For `LLM Polish`

1. Make sure `Polish Model` is assigned in `Settings > LLM Service`.
2. Click the `LLM Polish` button.
3. Choose the action you need:
   `LLM Polish`, `Re-transcribe`, `Undo`, `Redo`, or `Advanced Settings`.
4. Open `Advanced Settings` if you want to manage `Auto-Polish`, `Auto-Polish Frequency`, `Keywords`, `Scenario Presets`, or `Custom Context`.

### Steps For `Translate`

1. Make sure `Translation Model` is assigned in `Settings > LLM Service`.
2. Click the `Translate` button.
3. Choose the target language.
4. Click `Start Translation` or `Retranslate`.
5. Use `Show Translations` or `Hide Translations` to control bilingual display in the editor.

### Steps For `AI Summary`

1. Make sure `AI Summary` is enabled and `Summary Model` is assigned in `Settings > LLM Service`.
2. Open any transcript that already has segments.
3. Open the summary editor from the transcript surface.
4. Switch between `General`, `Meeting`, or `Lecture` if you want the next generation to use a different summary template.
5. Click `Generate` to create the current summary, or `Regenerate` to replace it later.
6. Edit the summary directly whenever you want to adjust wording. Sona auto-saves when focus leaves the editor.
7. Click `Copy` when you want to reuse the summary elsewhere.

### Result

- `LLM Polish` updates transcript text in place.
- `Translate` stores translation text per segment and can display it under the original text.
- `AI Summary` keeps one current summary record beside the transcript without changing the original text.

### Notes

- `Polish Model`, `Translation Model`, and `Summary Model` are configured separately. One provider can serve all three, or you can split them.
- Translation can use dedicated translation providers such as `Google Translate (Free)` or `Google Translate (API)`, but `LLM Polish` needs an LLM-capable provider and model.
- `AI Summary` also needs an LLM-capable provider and model; the Google Translate providers are not supported for summaries.
- You can turn `AI Summary` off in `Settings > LLM Service`. This hides the panel and prevents new summaries from being generated, but existing summary sidecar data is kept.
- If `AI Summary` is not fully configured yet, you can still open the summary editor and write or revise the text manually.
- Sona keeps one current summary record per transcript. Changing templates changes the next generation target or project default; it does not create a separate saved summary for every template.
- Translation target languages currently include `Chinese (Simplified)`, `English`, `Japanese`, `Korean`, `French`, `German`, and `Spanish`.
- `Re-transcribe` is only available when the current transcript came from a saved workspace item.
- Summary output is editable in the editor. You can revise it directly, let Sona auto-save it, and use the panel's copy button when you need it elsewhere.

## 8. Export Transcript

Export is available when the transcript contains at least one segment.

### Preconditions

- You have transcript content in the editor.

### Steps

1. Click the `Export` button in the header.
2. In the `Export Transcript` modal, enter a `Filename`.
3. Choose an `Export Directory`.
4. Pick an output format:
   `SubRip (.srt)`, `WebVTT (.vtt)`, `JSON (.json)`, or `Plain Text (.txt)`.
5. Choose an export mode:
   `Original`, `Translation`, or `Bilingual`.
6. Click `Export`.

### Result

- Sona writes the transcript to the selected path and format.
- If translations exist, you can export translated-only or bilingual output.

### Notes

- `Translation` and `Bilingual` modes are only available when at least one segment contains translation text.
- `Original` mode is always available.

## 9. Workspace, Projects, And Inbox

Use `Workspace` when you want to organize saved recordings and imports without leaving the main editing surface.

### What Each Scope Means

- `All Items` gives you a cross-project overview of everything saved in `Inbox` and every project.
- `Inbox` holds recordings and imports that are not assigned to a project yet.
- A project gives you its own focused workspace, project-specific defaults, and quick entry points for new work.

### Steps

1. Open `Workspace`.
2. Use the left rail to switch between `All Items`, `Inbox`, and any named project.
3. Click `New Project` when you want a dedicated space for a client, course, meeting series, or other recurring work.
4. Enter the project name and optional description, then create it. New projects start without a custom icon.
5. Open a project when you want new recordings or imports to stay attached to that project.
6. Use the project header buttons to start `Live Record` or open `Batch Import` while keeping the current project context.
7. Click any saved item to open it in the built-in detail pane and continue editing without leaving `Workspace`.
8. Use search, filters, sort, and `List View` / `Grid View` / `Table View` to narrow the current scope.
9. Turn on selection mode if you want to move items between `Inbox` and projects, or delete several items at once.

### Project Settings

1. Open a project, then click `Project Settings`.
2. Update the project name and description.
3. Choose a project icon if you want one. Icons are edited here, not in the `New Project` modal.
4. Pick the project defaults that should apply whenever you work inside that project:
   `Default Summary Template`, `Default Translation Language`, `Default Polish Scenario`, optional `Default Polish Context`, and `Export Filename Prefix`.
5. Turn `Text Replacement` and `Hotword` sets on or off for this project when you want project-specific recognition or cleanup behavior.
6. Click `Save` to keep the changes, or `Delete Project` if you want to remove the project and move its items back to `Inbox`.

### Result

- `Workspace` becomes the main place to organize saved content while keeping the editor close at hand.
- Project-specific defaults apply whenever that project is active.
- New work created from inside a project keeps that project context unless you move it later.

### Notes

- `All Items` is for browsing across saved content. It does not replace the current project's defaults.
- `Inbox` is the default landing place for new recordings and imports when you are not working inside a project.
- The active live recording session can already appear here as a `Draft` item while capture is still running. When you stop recording, that same item becomes the finished saved entry.
- Project icons support system icons, recommended emoji, and custom emoji.
- Saved recordings and imports also support title and icon changes through their own rename flow, but that is separate from `Project Settings`.
- If you switch away from a project while its settings drawer has unsaved changes, Sona asks whether to discard those edits first.

## 10. Export And Settings

Use `Export` when you are ready to write files out of Sona, and use `Settings` to manage app-wide defaults. If you want to reopen or organize saved work, return to `Workspace` instead of looking for a separate saved-items surface.

### Export Reminder

- Use the `Export` button in the header when the transcript is ready.
- Section `8. Export Transcript` above lists the formats and modes Sona currently supports.

### Settings Areas To Review

- `Settings > Dashboard`
  global content overview, speaker coverage, and LLM usage trends
- `Settings > General`
  theme, app language, font, tray behavior, update checks, plus `Diagnostics` and `Backup & Restore`
- `Settings > Input Device`
  microphone selection, system audio selection, microphone boost, mute during recording
- `Settings > Subtitle Settings`
  live caption startup, click-through lock, always-on-top, font size, width, color, background transparency
- `Settings > Voice Typing`
  enable `Voice Typing`, assign its global shortcut, choose `Push to Talk (Hold)` or `Toggle (Press once)`, and check readiness
- `Settings > Model Settings`
  `Live Record Model`, `Batch Import Model`, `Transcription Settings`, `ITN`, `VAD Buffer Size`, `Max Concurrent Transcriptions`, `Restore Default Settings`, and downloadable recognition, punctuation, speaker, and VAD models
- `Settings > Vocabulary`
  `Text Replacement`, `Hotwords`, polish keyword sets, polish context presets, and summary templates
- `Settings > Automation`
  watched-folder rules that can transcribe, polish, translate, and export new media while Sona is running
- `Settings > LLM Service`
  feature model bindings and provider credentials
- `Settings > Shortcuts`
  live recording shortcuts
- `Settings > About`
  source code, logs, and update-related actions

### `Voice Typing`

- `Voice Typing` is useful when you want to dictate directly into chat apps, documents, forms, or other applications.
- Open `Settings > Voice Typing`, turn on `Voice Typing`, choose a global shortcut, and pick either `Push to Talk (Hold)` or `Toggle (Press once)`.
- `Push to Talk (Hold)` is better for short bursts. `Toggle (Press once)` is better for longer dictation sessions.
- `Voice Typing` depends on the same offline live transcription setup, so you also need a working `Live Record Model`, any required `VAD` model, and an available input device.
- The same page also shows whether the current blocker is the shortcut, model, VAD, input device, or background warm-up state.

### Diagnostics And Backup

- In `Settings > General`, use `Diagnostics` to inspect the local transcription chain, runtime readiness, and packaging environment.
- In the same page, use `Backup & Restore` to export or import a light archive of config, workspace, light history transcripts and summaries, automation state, and dashboard LLM usage.
- `WebDAV Cloud Sync` lives inside `Backup & Restore`. It stores credentials locally on this device and helps you upload or restore backup archives manually.

### Notification Center

- The header notification center groups updates, `Recovery Center`, and automation results in one place.
- Use it when Sona detects interrupted batch or automation work, when an update is ready to relaunch, or when an automation rule succeeds or fails.
- `Recovery Center` lets you resume or discard interrupted batch and automation items. `Diagnostics` and `Backup & Restore` stay under `Settings > General`.

### Notes

- In `Settings > Vocabulary`, `Hotwords` are entered one per line. Weighted entries such as `Term :2.0` are supported, and hotwords are currently most relevant for Transducer and Qwen3 ASR models.
- In `Settings > Vocabulary`, `Text Replacement` can fix repeated terminology or spelling after transcription.

## 11. FAQ And Troubleshooting

### Sona keeps asking me to finish setup

- Open the onboarding banner and complete the model and microphone steps.
- If you skipped setup earlier, make sure both `Live Record Model` and `Batch Import Model` are configured in `Settings > Model Settings`.
- If you hid the reminder banner, reopen setup manually from the banner flow or settings-related entry points when Sona prompts again.

### `Live Record` does not start

- Check microphone permission in your operating system.
- Confirm a `Live Record Model` is configured.
- Make sure the input source is correct: `Microphone` or `Desktop Audio`.

### `Batch Import` does not start

- Make sure a `Batch Import Model` is configured.
- Confirm the file extension is supported.
- If Sona reports an unsupported format, convert the file first and try again.

### `LLM Polish` or `Translate` is disabled or fails

- Confirm `Settings > LLM Service` has the correct provider credentials.
- Make sure the feature itself has a model assigned: `Polish Model` for polishing, `Translation Model` for translation.
- Use `Test Connection` before retrying.
- If you use a custom endpoint or local service such as `Ollama`, verify that service first.

### I cannot find `Auto-Polish`

- Use `LLM Polish > Advanced Settings` for `Auto-Polish`, frequency, keywords, scenario presets, and custom context.

### Export only shows `Original`

- `Translation` and `Bilingual` only appear when the transcript already contains translation text.

### Why do new items appear in `Inbox` first

- `Inbox` is the default holding area for recordings and imports that are not assigned to a project yet.
- Open a specific project before starting `Live Record` or `Batch Import` if you want new items to stay in that project automatically.
- You can also move saved items later from `Inbox` into a project from `Workspace`.

### What happens when I delete a project

- Deleting a project removes the project itself, but it does not delete the recordings or imports that were inside it.
- Sona moves those items back to `Inbox`, so you can reassign or continue using them later.

### Why do I see a `Draft` item while I am still recording

- During an active live recording, Sona can create a visible `Draft` item so the session already has a saved place in `Workspace`.
- When you stop recording, Sona completes that same item instead of creating a second saved copy.
- Treat the draft as the live session while capture is still running; the finished transcript becomes available from the same entry.

### `Live Caption` does not appear

- Go back to the `Live Record` page and make sure `Live Caption` is turned on there. `Settings > Subtitle Settings` only controls the window behavior and appearance.
- If you only want floating system-audio subtitles, you do not need to start recording first. Turning on `Live Caption` is enough.
- `Live Caption` depends on the same offline live transcription setup, so make sure a `Live Record Model` is configured.

### Voice Typing does not work

- Turn on `Voice Typing` in `Settings > Voice Typing`.
- Confirm the page shows the shortcut, model, VAD, input device, and readiness state you expect.
- Make sure a live transcription model is configured, because Voice Typing depends on the same offline transcription setup.

### Where do I recover interrupted batch or automation work

- Open the header notification center if Sona reports pending recovery items.
- Use `Recovery Center` to resume or discard interrupted batch and automation work.
- If you were looking for diagnostics or backup instead, go to `Settings > General`.

### Playback controls are missing

- The audio player only appears when the current transcript has an audio source available, such as a saved recording or processed file.

### I want to build or develop Sona

- Use the [project README](../README.md) for source builds and development commands.
