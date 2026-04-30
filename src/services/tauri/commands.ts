type DeepValueOf<T> = T extends string
  ? T
  : {
      [K in keyof T]: DeepValueOf<T[K]>;
    }[keyof T];

export const TauriCommand = {
  app: {
    extractTarBz2: 'extract_tar_bz2',
    downloadFile: 'download_file',
    cancelDownload: 'cancel_download',
    openLogFolder: 'open_log_folder',
    getRuntimeEnvironmentStatus: 'get_runtime_environment_status',
    getPathStatuses: 'get_path_statuses',
    hasActiveDownloads: 'has_active_downloads',
    forceExit: 'force_exit',
    updateTrayMenu: 'update_tray_menu',
    setMinimizeToTray: 'set_minimize_to_tray',
  },
  audio: {
    setSystemAudioMute: 'set_system_audio_mute',
    getSystemAudioDevices: 'get_system_audio_devices',
    startSystemAudioCapture: 'start_system_audio_capture',
    stopSystemAudioCapture: 'stop_system_audio_capture',
    setSystemAudioCapturePaused: 'set_system_audio_capture_paused',
    setMicrophoneBoost: 'set_microphone_boost',
    getMicrophoneDevices: 'get_microphone_devices',
    startMicrophoneCapture: 'start_microphone_capture',
    stopMicrophoneCapture: 'stop_microphone_capture',
    setMicrophoneCapturePaused: 'set_microphone_capture_paused',
  },
  history: {
    listItems: 'history_list_items',
    createLiveDraft: 'history_create_live_draft',
    completeLiveDraft: 'history_complete_live_draft',
    saveRecording: 'history_save_recording',
    saveImportedFile: 'history_save_imported_file',
    deleteItems: 'history_delete_items',
    loadTranscript: 'history_load_transcript',
    updateTranscript: 'history_update_transcript',
    updateItemMeta: 'history_update_item_meta',
    updateProjectAssignments: 'history_update_project_assignments',
    reassignProject: 'history_reassign_project',
    loadSummary: 'history_load_summary',
    saveSummary: 'history_save_summary',
    deleteSummary: 'history_delete_summary',
    resolveAudioPath: 'history_resolve_audio_path',
    openFolder: 'history_open_folder',
  },
  llm: {
    generateText: 'generate_llm_text',
    listModels: 'list_llm_models',
    polishTranscriptSegments: 'polish_transcript_segments',
    summarizeTranscript: 'summarize_transcript',
    translateTranscriptSegments: 'translate_transcript_segments',
  },
  recognizer: {
    init: 'init_recognizer',
    start: 'start_recognizer',
    stop: 'stop_recognizer',
    flush: 'flush_recognizer',
    feedAudioChunk: 'feed_audio_chunk',
    processBatchFile: 'process_batch_file',
  },
  automation: {
    replaceRuntimeRules: 'replace_automation_runtime_rules',
    scanRuntimeRule: 'scan_automation_runtime_rule',
    collectRuntimeRulePaths: 'collect_automation_runtime_rule_paths',
  },
  backup: {
    exportArchive: 'export_backup_archive',
    prepareImport: 'prepare_backup_import',
    applyPreparedImport: 'apply_prepared_history_import',
    disposePreparedImport: 'dispose_prepared_backup_import',
    webdavTestConnection: 'webdav_test_connection',
    webdavListBackups: 'webdav_list_backups',
    webdavUploadBackup: 'webdav_upload_backup',
    webdavDownloadBackup: 'webdav_download_backup',
  },
  speaker: {
    annotateSegmentsFromFile: 'annotate_speaker_segments_from_file',
    importProfileSample: 'import_speaker_profile_sample',
  },
  system: {
    setAuxWindowState: 'set_aux_window_state',
    getAuxWindowState: 'get_aux_window_state',
    clearAuxWindowState: 'clear_aux_window_state',
    injectText: 'inject_text',
    getMousePosition: 'get_mouse_position',
    getTextCursorPosition: 'get_text_cursor_position',
  },
} as const;

export type TauriCommandName = DeepValueOf<typeof TauriCommand>;
