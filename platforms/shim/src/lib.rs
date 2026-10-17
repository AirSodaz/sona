pub mod dispatcher;
pub mod executor;
pub mod installer;
pub mod registry;

pub use dispatcher::{detect_role, resolve_target_channel, resolve_target_executable};
pub use executor::execute;
pub use installer::{PathStatus, check_path_status, get_shim_dir, install_shims, uninstall_shims};
pub use registry::{
    Channel, ChannelInfo, TargetRole, detect_active_ipc_channel, get_default_channel,
    get_registered_channel, set_default_channel,
};
