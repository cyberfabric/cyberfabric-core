// @cpt-component:cpt-cf-ues-component-modkit-errors:p1
//! Core error types for the modkit framework
//!
//! This crate provides pure data types for error handling, with no dependencies
//! on HTTP frameworks. It includes:
//! - RFC 9457 Problem Details (`Problem`)
//! - Error catalog support (`ErrDef`)
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

pub mod catalog;
pub mod problem;

// Re-export commonly used types
pub use catalog::ErrDef;
pub use problem::{
    APPLICATION_PROBLEM_JSON, Problem, ValidationError, ValidationErrorResponse,
    ValidationViolation,
};

/// Helper to attach instance and `trace_id` to a Problem
///
/// This is a convenience function for enriching Problem instances with
/// request-specific context before returning them as HTTP responses.
pub fn finalize(mut p: Problem, instance: &str, trace_id: Option<String>) -> Problem {
    p = p.with_instance(instance);
    if let Some(tid) = trace_id {
        p = p.with_trace_id(tid);
    }
    p
}
