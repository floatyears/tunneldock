use std::sync::OnceLock;

use tokio::sync::Mutex;

/// Global lock for package-manager/toolchain mutations.
///
/// Install and uninstall are both destructive operations on shared PATH, Cargo,
/// npm, winget/Homebrew state. They must never run concurrently, even when the
/// frontend fires requests from different component cards.
static COMPONENT_OPERATION_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

pub fn component_operation_lock() -> &'static Mutex<()> {
    COMPONENT_OPERATION_LOCK.get_or_init(|| Mutex::new(()))
}
