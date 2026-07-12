pub trait UnixMillisClock: Send + Sync {
    fn now_ms(&self) -> Result<u64, String>;
}
