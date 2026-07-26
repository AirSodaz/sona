use std::fmt;
use std::sync::Arc;

/// Opaque holder for a master password, provider password, or recovery key.
///
/// This is an object rather than a `String` field on purpose. UniFFI renders a
/// `Record` as a Kotlin `data class`, whose generated `toString()` prints every
/// field, so a credential carried as a plain field would leak into any log line
/// that formats the request. An object is rendered as a handle class whose
/// `toString()` is its identity, and the hand-written `Debug` below keeps Rust
/// logs equally safe. Mirrors `FfiOnlineAsrApiKey`.
#[derive(uniffi::Object)]
pub struct FfiSecret {
    value: String,
}

/// Value equality so records embedding a secret keep deriving `PartialEq`.
/// This is a plain comparison, not a constant-time one: it exists for config
/// records and tests, and must not be used to verify a credential.
impl PartialEq for FfiSecret {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

impl Eq for FfiSecret {}

impl fmt::Debug for FfiSecret {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FfiSecret")
            .field("value", &"<redacted>")
            .finish()
    }
}

#[uniffi::export]
impl FfiSecret {
    #[uniffi::constructor]
    pub fn new(value: String) -> Arc<Self> {
        Arc::new(Self { value })
    }
}

impl FfiSecret {
    pub(crate) fn expose(&self) -> &str {
        &self.value
    }
}
