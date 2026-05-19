use serde_json::Value;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

use super::status::*;

#[derive(Clone, Debug, Default)]
pub(super) struct DbConnection {
    pub(super) tables: HashMap<String, DbTable>,
}

#[derive(Clone, Debug)]
pub(super) struct DbTable {
    pub(super) columns: Vec<String>,
    pub(super) rows: Vec<Vec<Value>>,
}

#[derive(Clone, Debug, Default)]
pub(super) struct DbQueryResult {
    pub(super) columns: Vec<String>,
    pub(super) rows: Vec<Vec<Value>>,
}

#[derive(Clone, Debug)]
pub(super) struct DbErrorState {
    pub(super) code: i32,
    pub(super) message: String,
}

impl Default for DbErrorState {
    fn default() -> Self {
        Self {
            code: SENGOO_DB_STATUS_OK,
            message: String::new(),
        }
    }
}

static NEXT_DB_HANDLE: AtomicU64 = AtomicU64::new(1);
static DB_CONNECTIONS: OnceLock<Mutex<HashMap<u64, DbConnection>>> = OnceLock::new();
static DB_RESULTS: OnceLock<Mutex<HashMap<u64, DbQueryResult>>> = OnceLock::new();
static DB_LAST_ERROR: OnceLock<Mutex<DbErrorState>> = OnceLock::new();

pub(super) fn db_connections() -> &'static Mutex<HashMap<u64, DbConnection>> {
    DB_CONNECTIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

pub(super) fn db_results() -> &'static Mutex<HashMap<u64, DbQueryResult>> {
    DB_RESULTS.get_or_init(|| Mutex::new(HashMap::new()))
}

pub(super) fn db_last_error() -> &'static Mutex<DbErrorState> {
    DB_LAST_ERROR.get_or_init(|| Mutex::new(DbErrorState::default()))
}

pub(super) fn next_handle() -> u64 {
    NEXT_DB_HANDLE.fetch_add(1, Ordering::Relaxed)
}

pub(super) fn clear_error() {
    if let Ok(mut state) = db_last_error().lock() {
        state.code = SENGOO_DB_STATUS_OK;
        state.message.clear();
    }
}

pub(super) fn set_error(code: i32, message: impl Into<String>) -> i32 {
    if let Ok(mut state) = db_last_error().lock() {
        state.code = code;
        state.message = message.into();
    }
    code
}
