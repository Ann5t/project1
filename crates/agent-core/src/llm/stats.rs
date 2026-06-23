//! Global LLM API call counters for monitoring.
//!
//! These are globals so they work across the `LlmClient` trait boundary
//! without requiring trait method changes.

use std::sync::atomic::{AtomicU64, Ordering};

static LLM_CALL_TOTAL: AtomicU64 = AtomicU64::new(0);
static LLM_CALL_SUCCESS: AtomicU64 = AtomicU64::new(0);
static LLM_CALL_ERROR: AtomicU64 = AtomicU64::new(0);

/// Record that an LLM API call was initiated.
pub fn record_llm_call() {
    LLM_CALL_TOTAL.fetch_add(1, Ordering::Relaxed);
}

/// Record that an LLM API call completed successfully.
pub fn record_llm_success() {
    LLM_CALL_SUCCESS.fetch_add(1, Ordering::Relaxed);
}

/// Record that an LLM API call failed.
pub fn record_llm_error() {
    LLM_CALL_ERROR.fetch_add(1, Ordering::Relaxed);
}

/// Return current LLM stats as (total, success, error).
pub fn get_llm_stats() -> (u64, u64, u64) {
    (
        LLM_CALL_TOTAL.load(Ordering::Relaxed),
        LLM_CALL_SUCCESS.load(Ordering::Relaxed),
        LLM_CALL_ERROR.load(Ordering::Relaxed),
    )
}

/// Reset all LLM counters to zero.
pub fn reset_llm_stats() {
    LLM_CALL_TOTAL.store(0, Ordering::Relaxed);
    LLM_CALL_SUCCESS.store(0, Ordering::Relaxed);
    LLM_CALL_ERROR.store(0, Ordering::Relaxed);
}
