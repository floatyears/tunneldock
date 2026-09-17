use std::sync::OnceLock;

use tokio::sync::Mutex;

/// Serialize every package-manager/toolchain mutation across install and uninstall.
/// Frontend disabling is only UX; this lock is the backend safety boundary against
/// overlapping Tauri IPC calls.
static COMPONENT_OPERATION_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

pub fn component_operation_lock() -> &'static Mutex<()> {
    COMPONENT_OPERATION_LOCK.get_or_init(|| Mutex::new(()))
}
