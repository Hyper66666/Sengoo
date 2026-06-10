//! # Sengoo Runtime
//!
//! Sengoo runtime crate.

pub mod async_runtime;
pub mod error;
// `net` stays available in every build (including the `native-bridge`
// staticlib linked by `sgc`) so compiled Sengoo programs get the real
// network/HTTP server implementation instead of the C fallback stubs.
pub mod net;
// `reflect` ships C implementations in `tools/stdlib/runtime.c` for the sgc
// link path, so the Rust twin stays out of the `native-bridge` staticlib to
// avoid duplicate-symbol clashes.
#[cfg(not(feature = "native-bridge"))]
pub mod reflect;

#[cfg(feature = "python")]
pub mod python;

pub use async_runtime::{CoroutineScheduler, CoroutineTask, SchedulerStats, TaskId, TaskState};
pub use error::{Result, RuntimeError};
#[cfg(not(feature = "native-bridge"))]
pub use reflect::{
    ReflectInvokeError, ReflectValue, ReflectionLoadError, ReflectionMetadata,
    ReflectionModuleMetadata, ReflectionRuntime, ReflectionSymbolMetadata,
    REFLECTION_SCHEMA_VERSION,
};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
