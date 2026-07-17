use super::{TaskLedgerError, types::TaskLedgerRecord};

pub trait TaskLedgerStore: Send + Sync {
    fn load_records(&self) -> Result<Vec<TaskLedgerRecord>, TaskLedgerError>;

    fn upsert_record(&self, record: &TaskLedgerRecord) -> Result<(), TaskLedgerError>;

    fn update_record(
        &self,
        id: &str,
        update: &mut dyn FnMut(TaskLedgerRecord) -> Result<TaskLedgerRecord, TaskLedgerError>,
    ) -> Result<(), TaskLedgerError>;

    fn remove_record(&self, id: &str) -> Result<(), TaskLedgerError>;

    fn remove_records_matching(
        &self,
        predicate: &mut dyn FnMut(&TaskLedgerRecord) -> bool,
    ) -> Result<(), TaskLedgerError>;
}
