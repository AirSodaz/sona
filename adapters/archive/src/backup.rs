use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{self, BufReader, BufWriter, Read};
use std::path::Path;
use std::sync::{Arc, Mutex};

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use sona_core::automation::repository::AutomationRepositoryState;
use sona_core::backup::{
    BackupArchivePort, BackupDataset, BackupError, BackupManifest, PreparedBackupImport,
    PreparedBackupSession, validate_backup_manifest,
};
use sona_core::history::{
    HistoryBackupSnapshot, HistoryItemRecord, HistoryItemStatus, TranscriptSnapshotMetadata,
    TranscriptSnapshotRecord,
};
use sona_core::project::ProjectRecord;
use uuid::Uuid;

pub const MAX_BACKUP_ENTRIES: usize = 100_000;
pub const MAX_BACKUP_FILE_BYTES: u64 = 64 * 1024 * 1024;
pub const MAX_BACKUP_EXPANDED_BYTES: u64 = 4 * 1024 * 1024 * 1024;

const MAX_STRUCTURED_JSON_BYTES: u64 = 128 * 1024 * 1024;
const MAX_JSON_STRUCTURAL_TOKENS: u64 = 1_000_000;

const MANIFEST_PATH: &str = "manifest.json";
const CONFIG_PATH: &str = "config/sona-config.json";
const PROJECTS_PATH: &str = "projects/index.json";
const HISTORY_INDEX_PATH: &str = "history/index.json";
const AUTOMATION_RULES_PATH: &str = "automation/rules.json";
const AUTOMATION_PROCESSED_PATH: &str = "automation/processed.json";
const ANALYTICS_PATH: &str = "analytics/llm-usage.json";
const TAR_BLOCK_BYTES: usize = 512;
const TAR_SIZE_RANGE: std::ops::Range<usize> = 124..136;
const TAR_CHECKSUM_RANGE: std::ops::Range<usize> = 148..156;
const TAR_TYPEFLAG_OFFSET: usize = 156;

const FIXED_PATHS: [&str; 7] = [
    MANIFEST_PATH,
    CONFIG_PATH,
    PROJECTS_PATH,
    HISTORY_INDEX_PATH,
    AUTOMATION_RULES_PATH,
    AUTOMATION_PROCESSED_PATH,
    ANALYTICS_PATH,
];

struct PreparedBackupWorkspace {
    workspace: tempfile::TempDir,
}

#[derive(Clone, Default)]
pub struct FsBackupArchiveRepository {
    prepared: Arc<Mutex<HashMap<String, PreparedBackupWorkspace>>>,
}

impl FsBackupArchiveRepository {
    pub fn new() -> Self {
        Self::default()
    }

    fn lock_prepared(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, HashMap<String, PreparedBackupWorkspace>>, BackupError>
    {
        self.prepared
            .lock()
            .map_err(|error| BackupError::State(format!("Prepared backup sessions: {error}")))
    }

    fn take_prepared(
        &self,
        import_id: &str,
    ) -> Result<Option<PreparedBackupWorkspace>, BackupError> {
        let workspace = self.lock_prepared()?.remove(import_id);
        Ok(workspace)
    }

    fn requeue_prepared(
        &self,
        import_id: &str,
        workspace: PreparedBackupWorkspace,
    ) -> Result<(), BackupError> {
        let mut prepared = self.lock_prepared()?;
        if prepared.contains_key(import_id) {
            drop(prepared);
            return Err(BackupError::State(format!(
                "Prepared backup session was replaced during cleanup: {import_id}"
            )));
        }
        prepared.insert(import_id.to_string(), workspace);
        Ok(())
    }

    fn finish_workspace_operation<T>(
        &self,
        import_id: &str,
        workspace: PreparedBackupWorkspace,
        primary_result: Result<T, BackupError>,
        cleanup: impl FnOnce(&Path) -> io::Result<()>,
    ) -> Result<T, BackupError> {
        match cleanup(workspace.workspace.path()) {
            Ok(()) => primary_result,
            Err(error) => {
                let cleanup_error = archive_error(error);
                let requeue_result = self.requeue_prepared(import_id, workspace);
                match primary_result {
                    Err(primary) => {
                        let _ = requeue_result;
                        Err(primary)
                    }
                    Ok(_) => {
                        requeue_result?;
                        Err(cleanup_error)
                    }
                }
            }
        }
    }

    fn prepare_import_with_limits(
        &self,
        archive_path: &str,
        limits: JsonLimits,
    ) -> Result<PreparedBackupImport, BackupError> {
        let source = Path::new(archive_path);
        let file_name = source
            .file_name()
            .filter(|name| !name.is_empty())
            .ok_or_else(|| {
                BackupError::InvalidRequest("Backup archive path must name a file.".to_string())
            })?;
        let prefix = format!(".{}.sona-prepare-", file_name.to_string_lossy());
        let workspace = tempfile::Builder::new()
            .prefix(&prefix)
            .tempdir()
            .map_err(archive_error)?;
        let private_archive = workspace.path().join("source.sona-backup");
        fs::copy(source, &private_archive).map_err(archive_error)?;

        let paths = inspect_archive(&private_archive)?;
        let extraction_dir = workspace.path().join("contents");
        fs::create_dir(&extraction_dir).map_err(archive_error)?;
        unpack_archive(&private_archive, &extraction_dir)?;
        validate_structured_json_budget(&extraction_dir, &paths, limits)?;

        let import_id = Uuid::new_v4().to_string();
        let session = parse_prepared_session(&extraction_dir, &paths, import_id.clone())?;
        let preview = session_into_preview(session, archive_path.to_string())?;
        self.lock_prepared()?
            .insert(import_id, PreparedBackupWorkspace { workspace });
        Ok(preview)
    }
}

impl BackupArchivePort for FsBackupArchiveRepository {
    fn write_archive(
        &self,
        archive_path: &str,
        manifest: &BackupManifest,
        dataset: &BackupDataset,
    ) -> Result<(), BackupError> {
        let output = Path::new(archive_path);
        let entries = build_archive_entries(manifest, dataset)?;
        write_archive_atomically(output, &entries)
    }

    fn prepare_import(&self, archive_path: &str) -> Result<PreparedBackupImport, BackupError> {
        self.prepare_import_with_limits(archive_path, JsonLimits::default())
    }

    fn load_prepared(&self, import_id: &str) -> Result<PreparedBackupSession, BackupError> {
        let prepared = self.take_prepared(import_id)?.ok_or_else(|| {
            BackupError::InvalidRequest(format!(
                "Prepared backup import does not exist: {import_id}"
            ))
        })?;
        let primary_result = (|| {
            let private_archive = prepared.workspace.path().join("source.sona-backup");
            let extraction_dir = prepared.workspace.path().join("contents");
            let paths = inspect_archive(&private_archive)?;
            validate_structured_json_budget(&extraction_dir, &paths, JsonLimits::default())?;
            parse_prepared_session(&extraction_dir, &paths, import_id.to_string())
        })();
        self.finish_workspace_operation(
            import_id,
            prepared,
            primary_result,
            remove_prepared_workspace,
        )
    }

    fn dispose_prepared(&self, import_id: &str) -> Result<(), BackupError> {
        let Some(prepared) = self.take_prepared(import_id)? else {
            return Ok(());
        };
        self.finish_workspace_operation(import_id, prepared, Ok(()), remove_prepared_workspace)
    }
}

fn remove_prepared_workspace(path: &Path) -> io::Result<()> {
    match fs::remove_dir_all(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        result => result,
    }
}

#[derive(Default)]
struct ExpandedSizeTracker {
    total: u64,
}

impl ExpandedSizeTracker {
    fn add(&mut self, size: u64) -> Result<(), BackupError> {
        let total = self
            .total
            .checked_add(size)
            .ok_or_else(|| invalid_backup("Backup expanded size exceeds the supported limit."))?;
        if total > MAX_BACKUP_EXPANDED_BYTES {
            return Err(invalid_backup(
                "Backup expanded size exceeds the supported limit.",
            ));
        }
        self.total = total;
        Ok(())
    }
}

#[derive(Clone, Copy)]
struct JsonLimits {
    max_bytes: u64,
    max_tokens: u64,
}

impl Default for JsonLimits {
    fn default() -> Self {
        Self {
            max_bytes: MAX_STRUCTURED_JSON_BYTES,
            max_tokens: MAX_JSON_STRUCTURAL_TOKENS,
        }
    }
}

struct JsonBudget {
    limits: JsonLimits,
    bytes: u64,
    tokens: u64,
}

impl JsonBudget {
    fn new(limits: JsonLimits) -> Self {
        Self {
            limits,
            bytes: 0,
            tokens: 0,
        }
    }

    fn add_bytes(&mut self, count: usize) -> Result<(), BackupError> {
        self.bytes = self
            .bytes
            .checked_add(u64::try_from(count).map_err(archive_error)?)
            .ok_or_else(|| invalid_backup("Backup structured JSON byte budget exceeded."))?;
        if self.bytes > self.limits.max_bytes {
            return Err(invalid_backup(
                "Backup structured JSON byte budget exceeded.",
            ));
        }
        Ok(())
    }

    fn add_token(&mut self) -> Result<(), BackupError> {
        self.tokens = self
            .tokens
            .checked_add(1)
            .ok_or_else(|| invalid_backup("Backup JSON structural-token budget exceeded."))?;
        if self.tokens > self.limits.max_tokens {
            return Err(invalid_backup(
                "Backup JSON structural-token budget exceeded.",
            ));
        }
        Ok(())
    }
}

fn validate_structured_json_budget(
    root: &Path,
    paths: &HashSet<String>,
    limits: JsonLimits,
) -> Result<(), BackupError> {
    let mut budget = JsonBudget::new(limits);
    for path in paths {
        let file = File::open(root.join(path)).map_err(archive_error)?;
        scan_json_stream(BufReader::new(file), &mut budget)?;
    }
    Ok(())
}

fn scan_json_stream(mut reader: impl Read, budget: &mut JsonBudget) -> Result<(), BackupError> {
    let mut buffer = [0u8; 8 * 1024];
    let mut in_string = false;
    let mut escaped = false;
    loop {
        let read = reader.read(&mut buffer).map_err(archive_error)?;
        if read == 0 {
            return Ok(());
        }
        budget.add_bytes(read)?;
        for byte in &buffer[..read] {
            if in_string {
                if escaped {
                    escaped = false;
                } else if *byte == b'\\' {
                    escaped = true;
                } else if *byte == b'"' {
                    in_string = false;
                }
            } else if *byte == b'"' {
                in_string = true;
            } else if matches!(*byte, b'{' | b'}' | b'[' | b']' | b',' | b':') {
                budget.add_token()?;
            }
        }
    }
}

fn inspect_archive(archive_path: &Path) -> Result<HashSet<String>, BackupError> {
    validate_raw_tar_headers(archive_path)?;

    let file = File::open(archive_path).map_err(archive_error)?;
    let decoder = bzip2::read::BzDecoder::new(file);
    let mut archive = tar::Archive::new(decoder);
    let entries = archive.entries().map_err(archive_error)?;
    let mut seen_paths = HashSet::new();
    let mut file_paths = HashSet::new();
    let mut count = 0usize;
    let mut expanded = ExpandedSizeTracker::default();

    for entry in entries {
        let entry = entry.map_err(archive_error)?;
        count = count
            .checked_add(1)
            .ok_or_else(|| invalid_backup("Backup contains too many entries."))?;
        if count > MAX_BACKUP_ENTRIES {
            return Err(invalid_backup("Backup contains too many entries."));
        }
        let entry_type = entry.header().entry_type();
        if !entry_type.is_file() && !entry_type.is_dir() {
            return Err(invalid_backup(
                "Backup contains an unsupported non-regular entry.",
            ));
        }

        let size = entry.header().size().map_err(archive_error)?;
        if size > MAX_BACKUP_FILE_BYTES {
            return Err(invalid_backup(
                "Backup entry exceeds the supported file size limit.",
            ));
        }
        expanded.add(size)?;

        let path = normalize_archive_path(entry.path_bytes().as_ref())?;
        if !seen_paths.insert(path.clone()) {
            return Err(invalid_backup(format!(
                "Backup contains a duplicate path: {path}"
            )));
        }
        if entry_type.is_file() {
            file_paths.insert(path);
        }
    }

    Ok(file_paths)
}

fn validate_raw_tar_headers(archive_path: &Path) -> Result<(), BackupError> {
    let file = File::open(archive_path).map_err(archive_error)?;
    let mut decoder = bzip2::read::MultiBzDecoder::new(file);
    let mut count = 0usize;
    let mut expanded = ExpandedSizeTracker::default();

    loop {
        let header = read_tar_block(&mut decoder, "header")?;
        if header.iter().all(|byte| *byte == 0) {
            let terminator = read_tar_block(&mut decoder, "terminator")?;
            if terminator.iter().all(|byte| *byte == 0) {
                return validate_tar_trailing_padding(&mut decoder, &mut expanded);
            }
            return Err(invalid_backup(
                "Backup tar terminator contains a malformed header.",
            ));
        }

        validate_tar_checksum(&header)?;
        match header[TAR_TYPEFLAG_OFFSET] {
            0 | b'0' | b'5' => {}
            _ => {
                return Err(invalid_backup(
                    "Backup contains an unsupported non-regular entry.",
                ));
            }
        }

        let size = parse_tar_octal(&header[TAR_SIZE_RANGE], "size")?;
        count = count
            .checked_add(1)
            .ok_or_else(|| invalid_backup("Backup contains too many entries."))?;
        if count > MAX_BACKUP_ENTRIES {
            return Err(invalid_backup("Backup contains too many entries."));
        }
        if size > MAX_BACKUP_FILE_BYTES {
            return Err(invalid_backup(
                "Backup entry exceeds the supported file size limit.",
            ));
        }
        expanded.add(size)?;

        let padding =
            (TAR_BLOCK_BYTES as u64 - size % TAR_BLOCK_BYTES as u64) % TAR_BLOCK_BYTES as u64;
        let padded_size = size
            .checked_add(padding)
            .ok_or_else(|| invalid_backup("Backup tar entry size overflows."))?;
        let consumed = io::copy(&mut decoder.by_ref().take(padded_size), &mut io::sink())
            .map_err(archive_error)?;
        if consumed != padded_size {
            return Err(invalid_backup("Backup tar entry body is truncated."));
        }
    }
}

fn validate_tar_trailing_padding(
    reader: &mut impl Read,
    expanded: &mut ExpandedSizeTracker,
) -> Result<(), BackupError> {
    let mut buffer = [0u8; 8 * 1024];
    loop {
        let read = reader.read(&mut buffer).map_err(archive_error)?;
        if read == 0 {
            return Ok(());
        }
        expanded.add(read as u64)?;
        if buffer[..read].iter().any(|byte| *byte != 0) {
            return Err(invalid_backup(
                "Backup contains nonzero trailing data after the tar terminator.",
            ));
        }
    }
}

fn read_tar_block(
    reader: &mut impl Read,
    description: &str,
) -> Result<[u8; TAR_BLOCK_BYTES], BackupError> {
    let mut block = [0u8; TAR_BLOCK_BYTES];
    let mut offset = 0;
    while offset < block.len() {
        match reader.read(&mut block[offset..]).map_err(archive_error)? {
            0 => {
                return Err(invalid_backup(format!(
                    "Backup tar {description} is truncated."
                )));
            }
            read => offset += read,
        }
    }
    Ok(block)
}

fn validate_tar_checksum(header: &[u8; TAR_BLOCK_BYTES]) -> Result<(), BackupError> {
    let stored = parse_tar_octal(&header[TAR_CHECKSUM_RANGE], "checksum")?;
    let calculated = header
        .iter()
        .enumerate()
        .map(|(index, byte)| {
            if TAR_CHECKSUM_RANGE.contains(&index) {
                u64::from(b' ')
            } else {
                u64::from(*byte)
            }
        })
        .sum::<u64>();
    if stored != calculated {
        return Err(invalid_backup("Backup tar header checksum is invalid."));
    }
    Ok(())
}

fn parse_tar_octal(field: &[u8], description: &str) -> Result<u64, BackupError> {
    if field.first().is_some_and(|byte| byte & 0x80 != 0) {
        return Err(invalid_backup(format!(
            "Backup tar {description} uses unsupported base-256 encoding."
        )));
    }

    let bytes = field.iter().copied().skip_while(|byte| *byte == b' ');
    let mut value = 0u64;
    let mut saw_digit = false;
    let mut terminated = false;
    for byte in bytes {
        match byte {
            b'0'..=b'7' if !terminated => {
                saw_digit = true;
                value = value
                    .checked_mul(8)
                    .and_then(|current| current.checked_add(u64::from(byte - b'0')))
                    .ok_or_else(|| {
                        invalid_backup(format!("Backup tar {description} overflows."))
                    })?;
            }
            0 | b' ' if saw_digit => terminated = true,
            _ => {
                return Err(invalid_backup(format!(
                    "Backup tar {description} is not valid octal."
                )));
            }
        }
    }
    if !saw_digit {
        return Err(invalid_backup(format!(
            "Backup tar {description} is not valid octal."
        )));
    }
    Ok(value)
}

fn unpack_archive(archive_path: &Path, target: &Path) -> Result<(), BackupError> {
    let file = File::open(archive_path).map_err(archive_error)?;
    let decoder = bzip2::read::BzDecoder::new(file);
    let mut archive = tar::Archive::new(decoder);
    let entries = archive.entries().map_err(archive_error)?;

    for entry in entries {
        let mut entry = entry.map_err(archive_error)?;
        if !entry.unpack_in(target).map_err(archive_error)? {
            return Err(invalid_backup(
                "Backup entry could not be unpacked inside the extraction directory.",
            ));
        }
    }
    Ok(())
}

fn normalize_archive_path(path: &[u8]) -> Result<String, BackupError> {
    let path = std::str::from_utf8(path)
        .map_err(|_| invalid_backup("Backup entry path is not valid UTF-8."))?;
    if path.is_empty() || path.starts_with('/') || path.contains('\\') || path.contains('\0') {
        return Err(invalid_backup("Backup contains an unsafe entry path."));
    }

    let mut normalized = Vec::new();
    for component in path.split('/') {
        match component {
            "" | "." => continue,
            ".." => return Err(invalid_backup("Backup contains an unsafe entry path.")),
            value if value.contains(':') => {
                return Err(invalid_backup("Backup contains an unsafe entry path."));
            }
            value => normalized.push(value),
        }
    }
    if normalized.is_empty() {
        return Err(invalid_backup("Backup contains an unsafe entry path."));
    }
    Ok(normalized.join("/"))
}

fn parse_prepared_session(
    root: &Path,
    paths: &HashSet<String>,
    import_id: String,
) -> Result<PreparedBackupSession, BackupError> {
    for required in FIXED_PATHS {
        if !paths.contains(required) {
            return Err(invalid_backup(format!(
                "Backup is missing required entry: {required}"
            )));
        }
    }

    let manifest: BackupManifest = read_json(root, MANIFEST_PATH)?;
    validate_backup_manifest(&manifest)?;
    let config: Value = read_json(root, CONFIG_PATH)?;
    if !config.is_object() {
        return Err(invalid_backup("Backup config must be an object."));
    }
    let projects: Vec<ProjectRecord> = read_json(root, PROJECTS_PATH)?;
    let history_items: Vec<HistoryItemRecord> = read_json(root, HISTORY_INDEX_PATH)?;
    let automation_rules = read_json(root, AUTOMATION_RULES_PATH)?;
    let automation_processed_entries = read_json(root, AUTOMATION_PROCESSED_PATH)?;
    let analytics_content = fs::read_to_string(root.join(ANALYTICS_PATH)).map_err(archive_error)?;
    validate_analytics(&analytics_content)?;

    let mut expected_paths = FIXED_PATHS
        .into_iter()
        .map(ToOwned::to_owned)
        .collect::<HashSet<_>>();
    let history = parse_history(root, paths, &mut expected_paths, history_items)?;
    if &expected_paths != paths {
        let unexpected = paths
            .difference(&expected_paths)
            .next()
            .cloned()
            .unwrap_or_else(|| "unknown".to_string());
        return Err(invalid_backup(format!(
            "Backup contains an unexpected entry: {unexpected}"
        )));
    }

    let automation = AutomationRepositoryState {
        rules: automation_rules,
        processed_entries: automation_processed_entries,
    };
    let dataset = BackupDataset {
        config,
        projects,
        history,
        automation,
        analytics_content,
    };
    validate_counts(&manifest, &dataset)?;

    Ok(PreparedBackupSession {
        import_id,
        manifest,
        dataset,
    })
}

fn session_into_preview(
    session: PreparedBackupSession,
    archive_path: String,
) -> Result<PreparedBackupImport, BackupError> {
    let PreparedBackupSession {
        import_id,
        manifest,
        dataset,
    } = session;
    let BackupDataset {
        config,
        projects,
        history: _,
        automation,
        analytics_content,
    } = dataset;
    let automation_rules = to_values(&automation.rules)?;
    let automation_processed_entries = to_values(&automation.processed_entries)?;

    Ok(PreparedBackupImport {
        import_id,
        archive_path,
        manifest,
        config,
        projects: to_values(&projects)?,
        automation_rules,
        automation_processed_entries,
        analytics_content,
    })
}

fn parse_history(
    root: &Path,
    archive_paths: &HashSet<String>,
    expected_paths: &mut HashSet<String>,
    items: Vec<HistoryItemRecord>,
) -> Result<HistoryBackupSnapshot, BackupError> {
    let mut item_ids = HashSet::new();
    let mut transcript_files = Vec::with_capacity(items.len());
    let mut summary_files = Vec::new();
    let mut snapshot_files = Vec::new();

    for item in &items {
        if item.status == HistoryItemStatus::Draft {
            return Err(invalid_backup(format!(
                "Backup history item is a draft: {}",
                item.id
            )));
        }
        let item_id = safe_component(&item.id, "history item ID")?;
        if !item_ids.insert(item_id.clone()) {
            return Err(invalid_backup(format!(
                "Backup contains a duplicate history item ID: {item_id}"
            )));
        }

        let transcript_path = format!("history/{item_id}.json");
        expected_paths.insert(transcript_path.clone());
        let transcript: Value = read_json(root, &transcript_path)?;
        if !transcript.is_array() {
            return Err(invalid_backup(format!(
                "Backup transcript must be an array: {item_id}"
            )));
        }
        transcript_files.push((format!("{item_id}.json"), transcript));

        let summary_path = format!("history/{item_id}.summary.json");
        if archive_paths.contains(&summary_path) {
            expected_paths.insert(summary_path.clone());
            let summary: Value = read_json(root, &summary_path)?;
            if !summary.is_object() {
                return Err(invalid_backup(format!(
                    "Backup summary must be an object: {item_id}"
                )));
            }
            summary_files.push((item_id.clone(), summary));
        }
    }

    let mut archive_snapshot_paths = archive_paths
        .iter()
        .filter(|path| path.starts_with("history/versions/"))
        .cloned()
        .collect::<Vec<_>>();
    archive_snapshot_paths.sort();
    for archive_path in archive_snapshot_paths {
        expected_paths.insert(archive_path.clone());
        let snapshot = read_json(root, &archive_path)?;
        let relative_path = archive_path
            .strip_prefix("history/")
            .expect("snapshot archive path has history prefix")
            .to_string();
        snapshot_files.push((relative_path, snapshot));
    }
    validate_snapshot_set(&item_ids, &snapshot_files)?;

    Ok(HistoryBackupSnapshot {
        items,
        transcript_files,
        summary_files,
        snapshot_files,
    })
}

#[derive(Default)]
struct SnapshotSet<'a> {
    index: Option<&'a Value>,
    records: HashMap<String, &'a Value>,
}

fn validate_snapshot_set(
    known_history_ids: &HashSet<String>,
    snapshot_files: &[(String, Value)],
) -> Result<(), BackupError> {
    let mut paths = HashSet::new();
    let mut sets = HashMap::<String, SnapshotSet<'_>>::new();

    for (relative_path, value) in snapshot_files {
        let normalized = normalize_archive_path(relative_path.as_bytes())?;
        if normalized != *relative_path || !paths.insert(normalized.clone()) {
            return Err(invalid_backup(
                "Backup contains a duplicate or invalid snapshot path.",
            ));
        }
        let Some(rest) = normalized.strip_prefix("versions/") else {
            return Err(invalid_backup("Backup snapshot path is invalid."));
        };
        let Some((history_id, file_name)) = rest.split_once('/') else {
            return Err(invalid_backup("Backup snapshot path is invalid."));
        };
        if file_name.contains('/') || !known_history_ids.contains(history_id) {
            return Err(invalid_backup("Backup snapshot history ID is invalid."));
        }

        let set = sets.entry(history_id.to_string()).or_default();
        if file_name == "index.json" {
            set.index = Some(value);
            continue;
        }
        let Some(snapshot_id) = file_name.strip_suffix(".json") else {
            return Err(invalid_backup("Backup snapshot path is invalid."));
        };
        let snapshot_id = safe_component(snapshot_id, "snapshot ID")?;
        set.records.insert(snapshot_id, value);
    }

    for (history_id, set) in sets {
        let index = set.index.ok_or_else(|| {
            invalid_backup(format!(
                "Backup snapshot set is missing its index: {history_id}"
            ))
        })?;
        let metadata: Vec<TranscriptSnapshotMetadata> = serde_json::from_value(index.clone())
            .map_err(|error| invalid_backup(format!("Malformed backup JSON: {error}")))?;
        let mut expected_ids = HashSet::new();

        for expected_metadata in metadata {
            if expected_metadata.history_id != history_id {
                return Err(invalid_backup(format!(
                    "Backup snapshot history ID does not match: {}",
                    expected_metadata.id
                )));
            }
            let snapshot_id = safe_component(&expected_metadata.id, "snapshot ID")?;
            if !expected_ids.insert(snapshot_id.clone()) {
                return Err(invalid_backup(format!(
                    "Backup contains a duplicate snapshot ID: {snapshot_id}"
                )));
            }
            let record_value = set.records.get(&snapshot_id).ok_or_else(|| {
                invalid_backup(format!(
                    "Backup is missing indexed snapshot record: {snapshot_id}"
                ))
            })?;
            let record: TranscriptSnapshotRecord = serde_json::from_value((*record_value).clone())
                .map_err(|error| invalid_backup(format!("Malformed backup JSON: {error}")))?;
            if record.metadata != expected_metadata {
                return Err(invalid_backup(format!(
                    "Backup snapshot metadata does not match: {snapshot_id}"
                )));
            }
        }

        if let Some(unexpected) = set
            .records
            .keys()
            .find(|snapshot_id| !expected_ids.contains(*snapshot_id))
        {
            return Err(invalid_backup(format!(
                "Backup contains an unindexed snapshot record: {unexpected}"
            )));
        }
    }
    Ok(())
}

fn build_archive_entries(
    manifest: &BackupManifest,
    dataset: &BackupDataset,
) -> Result<Vec<(String, Vec<u8>)>, BackupError> {
    validate_backup_manifest(manifest)?;
    if !dataset.config.is_object() {
        return Err(invalid_backup("Backup config must be an object."));
    }
    validate_analytics(&dataset.analytics_content)?;
    validate_counts(manifest, dataset)?;

    let mut entries = vec![
        json_bytes(MANIFEST_PATH, manifest)?,
        json_bytes(CONFIG_PATH, &dataset.config)?,
        json_bytes(PROJECTS_PATH, &dataset.projects)?,
        json_bytes(HISTORY_INDEX_PATH, &dataset.history.items)?,
        json_bytes(AUTOMATION_RULES_PATH, &dataset.automation.rules)?,
        json_bytes(
            AUTOMATION_PROCESSED_PATH,
            &dataset.automation.processed_entries,
        )?,
        (
            ANALYTICS_PATH.to_string(),
            dataset.analytics_content.as_bytes().to_vec(),
        ),
    ];

    let item_ids = dataset
        .history
        .items
        .iter()
        .map(|item| safe_component(&item.id, "history item ID"))
        .collect::<Result<HashSet<_>, _>>()?;
    validate_snapshot_set(&item_ids, &dataset.history.snapshot_files)?;
    let expected_transcripts = item_ids
        .iter()
        .map(|id| format!("{id}.json"))
        .collect::<HashSet<_>>();
    let mut transcript_names = HashSet::new();
    for (file_name, transcript) in &dataset.history.transcript_files {
        let normalized = normalize_archive_path(file_name.as_bytes())?;
        if normalized.contains('/') || !transcript_names.insert(normalized.clone()) {
            return Err(invalid_backup("Backup transcript file name is invalid."));
        }
        if !transcript.is_array() {
            return Err(invalid_backup("Backup transcript must be an array."));
        }
        entries.push(json_bytes(&format!("history/{normalized}"), transcript)?);
    }
    if transcript_names != expected_transcripts {
        return Err(invalid_backup(
            "Backup transcripts do not match the history index.",
        ));
    }

    let mut summary_ids = HashSet::new();
    for (history_id, summary) in &dataset.history.summary_files {
        let history_id = safe_component(history_id, "summary history ID")?;
        if !item_ids.contains(&history_id) || !summary_ids.insert(history_id.clone()) {
            return Err(invalid_backup("Backup summary history ID is invalid."));
        }
        if !summary.is_object() {
            return Err(invalid_backup("Backup summary must be an object."));
        }
        entries.push(json_bytes(
            &format!("history/{history_id}.summary.json"),
            summary,
        )?);
    }

    for (relative_path, snapshot) in &dataset.history.snapshot_files {
        entries.push(json_bytes(&format!("history/{relative_path}"), snapshot)?);
    }

    validate_output_entries(&entries)?;
    Ok(entries)
}

fn validate_output_entries(entries: &[(String, Vec<u8>)]) -> Result<(), BackupError> {
    if entries.len() > MAX_BACKUP_ENTRIES {
        return Err(invalid_backup("Backup contains too many entries."));
    }
    let mut paths = HashSet::new();
    let mut expanded = ExpandedSizeTracker::default();
    let mut json_budget = JsonBudget::new(JsonLimits::default());
    for (path, bytes) in entries {
        let normalized = normalize_archive_path(path.as_bytes())?;
        if normalized != *path || !paths.insert(normalized) {
            return Err(invalid_backup(
                "Backup contains a duplicate or invalid path.",
            ));
        }
        let size = u64::try_from(bytes.len())
            .map_err(|_| invalid_backup("Backup entry size cannot be represented."))?;
        if size > MAX_BACKUP_FILE_BYTES {
            return Err(invalid_backup(
                "Backup entry exceeds the supported file size limit.",
            ));
        }
        let _ = regular_file_header(path, size)?;
        expanded.add(size)?;
        scan_json_stream(bytes.as_slice(), &mut json_budget)?;
    }
    Ok(())
}

fn write_archive_atomically(
    output: &Path,
    entries: &[(String, Vec<u8>)],
) -> Result<(), BackupError> {
    let file_name = output
        .file_name()
        .filter(|name| !name.is_empty())
        .ok_or_else(|| {
            BackupError::InvalidRequest("Backup archive path must name a file.".to_string())
        })?;
    let parent = output.parent().unwrap_or_else(|| Path::new("."));
    if !parent.as_os_str().is_empty() {
        fs::create_dir_all(parent).map_err(archive_error)?;
    }
    let staging = parent.join(format!(
        ".{}.sona-staging-{}",
        file_name.to_string_lossy(),
        Uuid::new_v4()
    ));

    let result = write_staging_archive(&staging, entries).and_then(|()| publish(&staging, output));
    if result.is_err() {
        let _ = fs::remove_file(&staging);
    }
    result
}

fn write_staging_archive(staging: &Path, entries: &[(String, Vec<u8>)]) -> Result<(), BackupError> {
    let file = File::create(staging).map_err(archive_error)?;
    let writer = BufWriter::new(file);
    let encoder = bzip2::write::BzEncoder::new(writer, bzip2::Compression::best());
    let mut builder = tar::Builder::new(encoder);
    for (path, bytes) in entries {
        let header = regular_file_header(path, bytes.len() as u64)?;
        builder
            .append(&header, bytes.as_slice())
            .map_err(archive_error)?;
    }
    let encoder = builder.into_inner().map_err(archive_error)?;
    let writer = encoder.finish().map_err(archive_error)?;
    let file = writer
        .into_inner()
        .map_err(|error| archive_error(error.into_error()))?;
    file.sync_all().map_err(archive_error)
}

fn regular_file_header(path: &str, size: u64) -> Result<tar::Header, BackupError> {
    let mut header = tar::Header::new_gnu();
    header.set_path(path).map_err(|error| {
        BackupError::Archive(format!(
            "Backup entry path cannot be represented directly in the tar header: {path}: {error}"
        ))
    })?;
    header.set_mode(0o600);
    header.set_uid(0);
    header.set_gid(0);
    header.set_mtime(0);
    header.set_size(size);
    header.set_entry_type(tar::EntryType::Regular);
    header.set_cksum();
    Ok(header)
}

#[cfg(not(windows))]
fn publish(staging: &Path, output: &Path) -> Result<(), BackupError> {
    fs::rename(staging, output).map_err(archive_error)
}

#[cfg(windows)]
fn publish(staging: &Path, output: &Path) -> Result<(), BackupError> {
    if !output.exists() {
        return fs::rename(staging, output).map_err(archive_error);
    }

    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{REPLACEFILE_WRITE_THROUGH, ReplaceFileW};

    let output_wide = output
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let staging_wide = staging
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let replaced = unsafe {
        ReplaceFileW(
            output_wide.as_ptr(),
            staging_wide.as_ptr(),
            std::ptr::null(),
            REPLACEFILE_WRITE_THROUGH,
            std::ptr::null(),
            std::ptr::null(),
        )
    };
    if replaced == 0 {
        return Err(archive_error(std::io::Error::last_os_error()));
    }
    Ok(())
}

fn validate_counts(manifest: &BackupManifest, dataset: &BackupDataset) -> Result<(), BackupError> {
    ensure_count(manifest.counts.projects, dataset.projects.len(), "project")?;
    ensure_count(
        manifest.counts.history_items,
        dataset.history.items.len(),
        "history",
    )?;
    ensure_count(
        manifest.counts.transcript_files,
        dataset.history.transcript_files.len(),
        "transcript",
    )?;
    ensure_count(
        manifest.counts.summary_files,
        dataset.history.summary_files.len(),
        "summary",
    )?;
    ensure_count(
        manifest.counts.automation_rules,
        dataset.automation.rules.len(),
        "automation-rule",
    )?;
    ensure_count(
        manifest.counts.automation_processed_entries,
        dataset.automation.processed_entries.len(),
        "processed-entry",
    )?;
    ensure_count(manifest.counts.analytics_files, 1, "analytics")
}

fn ensure_count(expected: u64, actual: usize, label: &str) -> Result<(), BackupError> {
    if expected == actual as u64 {
        return Ok(());
    }
    Err(invalid_backup(format!(
        "Backup {label} count does not match the manifest."
    )))
}

fn validate_analytics(content: &str) -> Result<(), BackupError> {
    match serde_json::from_str::<Value>(content)
        .map_err(|error| invalid_backup(format!("Malformed backup JSON: {error}")))?
    {
        Value::Object(_) | Value::Array(_) => Ok(()),
        _ => Err(invalid_backup(
            "Backup analytics must be an object or an array.",
        )),
    }
}

fn safe_component(value: &str, label: &str) -> Result<String, BackupError> {
    let normalized = normalize_archive_path(value.as_bytes())?;
    if normalized != value || normalized.contains('/') {
        return Err(invalid_backup(format!("Backup {label} is invalid.")));
    }
    Ok(normalized)
}

fn read_json<T: DeserializeOwned>(root: &Path, relative: &str) -> Result<T, BackupError> {
    let file = File::open(root.join(relative)).map_err(archive_error)?;
    serde_json::from_reader(BufReader::new(file))
        .map_err(|error| invalid_backup(format!("Malformed backup JSON: {error}")))
}

fn json_bytes<T: Serialize + ?Sized>(
    path: &str,
    value: &T,
) -> Result<(String, Vec<u8>), BackupError> {
    serde_json::to_vec_pretty(value)
        .map(|bytes| (path.to_string(), bytes))
        .map_err(|error| BackupError::Archive(error.to_string()))
}

fn to_values<T: Serialize>(values: &[T]) -> Result<Vec<Value>, BackupError> {
    values
        .iter()
        .map(|value| {
            serde_json::to_value(value).map_err(|error| BackupError::Archive(error.to_string()))
        })
        .collect()
}

fn invalid_backup(reason: impl Into<String>) -> BackupError {
    BackupError::InvalidBackup(reason.into())
}

fn archive_error(error: impl std::fmt::Display) -> BackupError {
    BackupError::Archive(error.to_string())
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::{
        ExpandedSizeTracker, FsBackupArchiveRepository, JsonBudget, JsonLimits,
        MAX_BACKUP_EXPANDED_BYTES, PreparedBackupWorkspace, remove_prepared_workspace,
        scan_json_stream, validate_output_entries, validate_tar_trailing_padding,
        write_archive_atomically,
    };
    #[test]
    fn rejects_total_expanded_size_before_it_exceeds_the_limit() {
        let mut tracker = ExpandedSizeTracker::default();
        for _ in 0..64 {
            tracker.add(64 * 1024 * 1024).unwrap();
        }

        let error = tracker.add(1).unwrap_err();
        assert!(error.to_string().contains("expanded size"));
    }

    #[test]
    fn json_budget_counts_bytes_and_only_structural_tokens_outside_strings() {
        let input = br#"{"text":"{,\"}","items":[]}"#;
        let mut budget = JsonBudget::new(JsonLimits {
            max_bytes: input.len() as u64,
            max_tokens: 7,
        });

        scan_json_stream(Cursor::new(input), &mut budget).unwrap();

        assert_eq!(budget.bytes, input.len() as u64);
        assert_eq!(budget.tokens, 7);
    }

    #[test]
    fn json_budget_rejects_small_byte_and_token_limits() {
        let mut byte_budget = JsonBudget::new(JsonLimits {
            max_bytes: 3,
            max_tokens: 10,
        });
        let byte_error = scan_json_stream(Cursor::new(b"null"), &mut byte_budget).unwrap_err();
        assert!(byte_error.to_string().contains("byte budget"));

        let mut token_budget = JsonBudget::new(JsonLimits {
            max_bytes: 10,
            max_tokens: 2,
        });
        let token_error =
            scan_json_stream(Cursor::new(br#"{"a":[]}"#), &mut token_budget).unwrap_err();
        assert!(token_error.to_string().contains("structural-token budget"));
    }

    #[test]
    fn output_validation_enforces_the_import_structural_token_budget() {
        let item_count = 333_334;
        let mut json = String::with_capacity(item_count * 3 + 2);
        json.push('[');
        for index in 0..item_count {
            if index > 0 {
                json.push(',');
            }
            json.push_str("[]");
        }
        json.push(']');

        let error =
            validate_output_entries(&[("analytics/llm-usage.json".to_string(), json.into_bytes())])
                .unwrap_err();

        assert!(error.to_string().contains("structural-token budget"));
    }

    #[test]
    fn trailing_zero_padding_counts_toward_the_expanded_size_limit() {
        let mut expanded = ExpandedSizeTracker {
            total: MAX_BACKUP_EXPANDED_BYTES - 1,
        };

        let error = validate_tar_trailing_padding(&mut Cursor::new([0_u8, 0_u8]), &mut expanded)
            .unwrap_err();

        assert!(error.to_string().contains("expanded size"));
    }

    #[test]
    fn budget_failure_happens_before_dom_parse_or_session_insertion() {
        let temp = tempfile::tempdir().unwrap();
        let archive_path = temp.path().join("budget.sona");
        write_archive_atomically(
            &archive_path,
            &[("manifest.json".to_string(), b"[[[".to_vec())],
        )
        .unwrap();
        let repository = FsBackupArchiveRepository::new();

        let error = repository
            .prepare_import_with_limits(
                archive_path.to_str().unwrap(),
                JsonLimits {
                    max_bytes: 10,
                    max_tokens: 2,
                },
            )
            .unwrap_err();

        assert!(error.to_string().contains("structural-token budget"));
        assert!(repository.lock_prepared().unwrap().is_empty());
    }

    fn repository_with_workspace() -> (FsBackupArchiveRepository, String, std::path::PathBuf) {
        let repository = FsBackupArchiveRepository::new();
        let import_id = "prepared-cleanup".to_string();
        let workspace = tempfile::tempdir().unwrap();
        let workspace_path = workspace.path().to_path_buf();
        repository
            .lock_prepared()
            .unwrap()
            .insert(import_id.clone(), PreparedBackupWorkspace { workspace });
        (repository, import_id, workspace_path)
    }

    #[test]
    fn cleanup_releases_the_lock_and_requeues_a_failure_for_retry() {
        let (repository, import_id, workspace_path) = repository_with_workspace();
        let workspace = repository.take_prepared(&import_id).unwrap().unwrap();

        let first: Result<(), _> =
            repository.finish_workspace_operation(&import_id, workspace, Ok(()), |_| {
                assert!(repository.prepared.try_lock().is_ok());
                Err(std::io::Error::other("injected cleanup failure"))
            });

        assert!(
            first
                .unwrap_err()
                .to_string()
                .contains("injected cleanup failure")
        );
        assert!(repository.lock_prepared().unwrap().contains_key(&import_id));

        let workspace = repository.take_prepared(&import_id).unwrap().unwrap();
        repository
            .finish_workspace_operation(&import_id, workspace, Ok(()), remove_prepared_workspace)
            .unwrap();
        assert!(!workspace_path.exists());
        assert!(!repository.lock_prepared().unwrap().contains_key(&import_id));
    }

    #[test]
    fn primary_materialization_error_wins_over_cleanup_failure() {
        let (repository, import_id, _) = repository_with_workspace();
        let workspace = repository.take_prepared(&import_id).unwrap().unwrap();
        let primary = sona_core::backup::BackupError::InvalidBackup("primary".to_string());

        let result: Result<(), _> = repository.finish_workspace_operation(
            &import_id,
            workspace,
            Err(primary.clone()),
            |_| Err(std::io::Error::other("cleanup")),
        );

        assert_eq!(result.unwrap_err(), primary);
        assert!(repository.lock_prepared().unwrap().contains_key(&import_id));
    }

    #[test]
    fn successful_materialization_propagates_cleanup_failure_and_requeues() {
        let (repository, import_id, _) = repository_with_workspace();
        let workspace = repository.take_prepared(&import_id).unwrap().unwrap();

        let result = repository.finish_workspace_operation(
            &import_id,
            workspace,
            Ok("materialized"),
            |_| Err(std::io::Error::other("cleanup after success")),
        );

        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("cleanup after success")
        );
        assert!(repository.lock_prepared().unwrap().contains_key(&import_id));
    }
}
