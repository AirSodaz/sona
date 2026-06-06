pub mod repository;
pub mod types;

pub use repository::AutomationRepository;
pub use repository::create_automation_fingerprint;
pub use repository::normalize_automation_path;
pub use repository::validate_rule_activation_inner;
pub use types::*;
