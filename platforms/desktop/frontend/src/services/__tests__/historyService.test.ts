import { beforeEach, describe, expect, it, vi } from "vitest";

const testContext = vi.hoisted(() => ({
  invokeMock: vi.fn(),
}));

vi.mock("@tauri-apps/api/core", () => ({
  convertFileSrc: vi.fn().mockImplementation((path) => `asset://${path}`),
  invoke: testContext.invokeMock,
}));

vi.mock("../../utils/logger", () => ({
  logger: {
    error: vi.fn(),
    info: vi.fn(),
  },
}));

import { historyService } from "../historyService";

describe("historyService", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("createLiveRecordingDraft lets Rust create the draft item and normalizes the returned item", async () => {
    testContext.invokeMock.mockResolvedValue({
      item: {
        id: "rust-history-1",
        timestamp: 123456789,
        duration: 0,
        audioPath: "rust-history-1.webm",
        transcriptPath: "rust-history-1.json",
        title: "Recording 1970-01-02 10-17-36",
        previewText: "",
        icon: "system:mic",
        type: "recording",
        searchContent: "",
        tagIds: [],
        deletedAt: null,
        status: "draft",
        draftSource: "live_record",
      },
      audioAbsolutePath: "C:\\AppData\\history\\rust-history-1.webm",
    });

    const result = await historyService.createLiveRecordingDraft(
      ".webm",
      "project-1",
      "system:mic",
    );

    expect(testContext.invokeMock).toHaveBeenCalledWith(
      "history_create_live_draft",
      {
        id: null,
        audioExtension: ".webm",
        tagIds: ["project-1"],
        icon: "system:mic",
      },
    );
    expect(result).toEqual(
      expect.objectContaining({
        audioAbsolutePath: "C:\\AppData\\history\\rust-history-1.webm",
        item: expect.objectContaining({
          id: "rust-history-1",
          status: "draft",
        }),
      }),
    );
  });

  it("saveRecording sends browser audio bytes as transport and leaves item creation to Rust", async () => {
    testContext.invokeMock.mockResolvedValue({
      id: "rust-recording-1",
      timestamp: 123456789,
      duration: 3,
      audioPath: "rust-recording-1.webm",
      transcriptPath: "rust-recording-1.json",
      title: "Recording 1970-01-02 10-17-36",
      previewText: "Hello...",
      type: "recording",
      searchContent: "Hello",
      tagIds: ["project-1"],
      deletedAt: null,
      status: "complete",
    });

    const item = await historyService.saveRecording(
      new Blob([new Uint8Array([1, 2, 3])], { type: "audio/webm" }),
      [{ id: "seg-1", text: "Hello", start: 0, end: 3, isFinal: true }],
      3,
      "project-1",
    );

    expect(testContext.invokeMock).toHaveBeenCalledWith(
      "history_save_recording",
      {
        segments: [
          { id: "seg-1", text: "Hello", start: 0, end: 3, isFinal: true },
        ],
        duration: 3,
        tagIds: ["project-1"],
        audioBytes: [1, 2, 3],
        audioExtension: "webm",
      },
    );
    expect(item).toEqual(
      expect.objectContaining({
        id: "rust-recording-1",
        previewText: "Hello...",
        searchContent: "Hello",
      }),
    );
  });

  it("saveNativeRecording sends the native path and leaves item creation to Rust", async () => {
    testContext.invokeMock.mockResolvedValue({
      id: "rust-native-1",
      timestamp: 123456789,
      duration: 4,
      audioPath: "capture.wav",
      transcriptPath: "rust-native-1.json",
      title: "Recording 1970-01-02 10-17-36",
      previewText: "Native...",
      type: "recording",
      searchContent: "Native",
      tagIds: [],
      deletedAt: null,
      status: "complete",
    });

    await historyService.saveNativeRecording(
      "C:\\AppData\\history\\capture.wav",
      [{ id: "seg-1", text: "Native", start: 0, end: 4, isFinal: true }],
      4,
    );

    expect(testContext.invokeMock).toHaveBeenCalledWith(
      "history_save_recording",
      {
        segments: [
          { id: "seg-1", text: "Native", start: 0, end: 4, isFinal: true },
        ],
        duration: 4,
        tagIds: [],
        nativeAudioPath: "C:\\AppData\\history\\capture.wav",
        audioExtension: "wav",
      },
    );
  });

  it("saveImportedFile preserves the original source path and passes converted source separately", async () => {
    testContext.invokeMock.mockResolvedValue({
      id: "rust-import-1",
      timestamp: 123456789,
      duration: 5,
      audioPath: "rust-import-1.wav",
      transcriptPath: "rust-import-1.json",
      title: "Batch meeting.mp3",
      previewText: "Imported...",
      type: "batch",
      searchContent: "Imported",
      tagIds: ["project-1"],
      deletedAt: null,
      status: "complete",
    });

    await historyService.saveImportedFile(
      "D:\\audio\\meeting.mp3",
      [{ id: "seg-1", text: "Imported", start: 0, end: 5, isFinal: true }],
      5,
      "C:\\Temp\\meeting.wav",
      "project-1",
    );

    expect(testContext.invokeMock).toHaveBeenCalledWith(
      "history_save_imported_file",
      {
        sourcePath: "D:\\audio\\meeting.mp3",
        segments: [
          { id: "seg-1", text: "Imported", start: 0, end: 5, isFinal: true },
        ],
        duration: 5,
        tagIds: ["project-1"],
        convertedSourcePath: "C:\\Temp\\meeting.wav",
        id: null,
      },
    );
  });

  it("deleteRecordings forwards the whole id set through one Rust command", async () => {
    testContext.invokeMock.mockResolvedValue(undefined);

    await historyService.deleteRecordings(["1", "3"]);

    expect(testContext.invokeMock).toHaveBeenCalledWith(
      "history_delete_items",
      {
        ids: ["1", "3"],
      },
    );
  });

  it("forwards history audio cleanup preview and apply requests", async () => {
    testContext.invokeMock
      .mockResolvedValueOnce({
        eligibleCount: 2,
        removedCount: 2,
        removedBytes: 123,
        missingMarkedCount: 0,
        failedCount: 0,
        skippedActiveCount: 1,
      })
      .mockResolvedValueOnce({
        eligibleCount: 1,
        removedCount: 0,
        removedBytes: 0,
        missingMarkedCount: 1,
        failedCount: 0,
        skippedActiveCount: 0,
      });

    const preview = await historyService.previewAudioCleanup(30, "history-open");
    const cleanup = await historyService.cleanupAudio(null, null);

    expect(testContext.invokeMock).toHaveBeenNthCalledWith(
      1,
      "history_preview_audio_cleanup",
      {
        retentionDays: 30,
        excludeHistoryId: "history-open",
      },
    );
    expect(testContext.invokeMock).toHaveBeenNthCalledWith(
      2,
      "history_cleanup_audio",
      {
        retentionDays: null,
        excludeHistoryId: null,
      },
    );
    expect(preview.removedBytes).toBe(123);
    expect(cleanup.missingMarkedCount).toBe(1);
  });

  it("loadTranscript trusts Rust-normalized timing from the Rust bridge", async () => {
    testContext.invokeMock.mockResolvedValue([
      {
        id: "seg-1",
        text: "你好",
        start: 0,
        end: 1,
        isFinal: true,
        tokens: ["你", "好"],
        timestamps: [0, 0.5],
      },
    ]);

    const segments = await historyService.loadTranscript("hist-1");

    expect(testContext.invokeMock).toHaveBeenCalledWith(
      "history_load_transcript",
      {
        historyId: "hist-1",
      },
    );
    expect(segments).toHaveLength(1);
    expect(segments?.[0].timing).toBeUndefined();
  });

  it("creates and loads transcript snapshots through the Rust bridge", async () => {
    testContext.invokeMock
      .mockResolvedValueOnce({
        id: "snapshot-1",
        historyId: "history-1",
        reason: "polish",
        createdAt: 1,
        segmentCount: 1,
      })
      .mockResolvedValueOnce({
        metadata: {
          id: "snapshot-1",
          historyId: "history-1",
          reason: "polish",
          createdAt: 1,
          segmentCount: 1,
        },
        segments: [
          {
            id: "seg-1",
            text: "你好",
            start: 0,
            end: 1,
            isFinal: true,
            tokens: ["你", "好"],
            timestamps: [0, 0.5],
          },
        ],
      });

    await historyService.createTranscriptSnapshot("history-1", "polish", [
      { id: "seg-1", text: "你好", start: 0, end: 1, isFinal: true },
    ]);
    const record = await historyService.loadTranscriptSnapshot(
      "history-1",
      "snapshot-1",
    );

    expect(testContext.invokeMock).toHaveBeenNthCalledWith(
      1,
      "history_create_transcript_snapshot",
      {
        historyId: "history-1",
        reason: "polish",
        segments: [
          { id: "seg-1", text: "你好", start: 0, end: 1, isFinal: true },
        ],
      },
    );
    expect(testContext.invokeMock).toHaveBeenNthCalledWith(
      2,
      "history_load_transcript_snapshot",
      {
        historyId: "history-1",
        snapshotId: "snapshot-1",
      },
    );
    expect(record?.segments[0].timing).toBeUndefined();
  });
});
