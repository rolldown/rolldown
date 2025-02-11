use crate::types::{defer_sync_scan_data::BindingDeferSyncScanData, js_callback::JsCallback};

pub type BindingDeferSyncScanDataOption = JsCallback<(), Vec<BindingDeferSyncScanData>>;
