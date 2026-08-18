//! Admission control: should this work start now, or wait?
//!
//! A loop, not a request handler — machines free up when jobs finish and
//! something must continuously notice and release more work.

#![warn(missing_docs)]

pub mod admission;
pub mod api;
pub mod capacity;
pub mod engine;
pub mod fairness;
pub mod model;
pub mod store;

#[cfg(feature = "temporal")]
pub mod temporal;
