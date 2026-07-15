//! Typed link passes and their narrow artifacts.

#![forbid(unsafe_code)]

mod compute_tla;

pub(super) use compute_tla::TlaScanFacts;

#[derive(Clone, Copy)]
pub(super) struct ComputeTlaPass;

#[cfg(test)]
mod inventory;
