use super::types::TaskLedgerRecord;

pub trait TaskLedgerStore: Send + Sync {
    fn load_records(&self) -> Result<Vec<TaskLedgerRecord>, String>;

    fn upsert_record(&self, record: &TaskLedgerRecord) -> Result<(), String>;

    fn update_record(
        &self,
        id: &str,
        update: &mut dyn FnMut(TaskLedgerRecord) -> Result<TaskLedgerRecord, String>,
    ) -> Result<(), String>;

    fn remove_record(&self, id: &str) -> Result<(), String>;

    fn remove_records_matching(
        &self,
        predicate: &mut dyn FnMut(&TaskLedgerRecord) -> bool,
    ) -> Result<(), String>;
}
