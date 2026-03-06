//! # Sengoo Runtime
//!
//! Sengoo runtime crate.

pub mod async_runtime;
pub mod error;
#[cfg(not(feature = "native-bridge"))]
pub mod net;
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
