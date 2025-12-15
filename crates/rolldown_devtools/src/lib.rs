//! Rolldown tries to emit useful internal tracing data during build time, so that devtools can consume them to provide better debugging experience.
//! This crate implements the mechanism to emit tracing data and some utilities to help with that.
//!
//! ## Implementation Details
//!
//! The mechanism is built on top of the `tracing` crate. There're two main important concepts in the tracing crate:
//!
//! - Spans: represent a period of time in the program execution. They can have fields associated with them.
//! - Events: represent a single point in time. They can also have fields associated with them
//!
//! ### Mental model
//!
//! The way how rolldown devtools tracing works is like
//!
//! ```ignore
//! <BundlerSpan>
//!   <TransformCallSpan CONTEXT_transform_call_id=".." CONTEXT_plugin_name="..">
//!     {emitTransformStartEvent({ transform_call_id: '${transform_call_id}', plugin_name: '${plugin_name}' })}
//!     {runTransformCode()}
//!     {emitTransformEndEvent({ transform_call_id: '${transform_call_id}', plugin_name: '${plugin_name}' })}
//!   </TransformCallSpan>
//! </BundlerSpan>
//! ```
//!
//! ### Why?
//!
//! - Spans allows us inject context data implicitly, so that we don't need to pass them around manually.
//! - Spans could track the async context automatically, so that we don't need to worry about losing context in async code.

mod devtools_formatter;
mod devtools_layer;
mod init_tracing;
mod static_data;
mod trace_action_macro;
mod type_alias;
mod types;
mod utils;

pub use rolldown_devtools_action as action;

pub use {
  init_tracing::{DebugTracer, Session},
  utils::{generate_build_id, generate_session_id},
};
