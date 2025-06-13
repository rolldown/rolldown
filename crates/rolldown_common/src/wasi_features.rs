use crate::Platform;

/// Checks if the given platform is any version of WASI
pub fn is_wasi_platform(platform: Platform) -> bool {
  matches!(platform, Platform::Wasi | Platform::WasiP2)
}

/// Checks if the platform is specifically WASI Preview 2
pub fn is_wasi_preview2(platform: Platform) -> bool {
  matches!(platform, Platform::WasiP2)
}

/// Returns the appropriate target triple for the given WASI platform
pub fn get_wasi_target_triple(platform: Platform) -> Option<&'static str> {
  match platform {
    Platform::Wasi => Some("wasm32-wasip1-threads"),
    Platform::WasiP2 => Some("wasm32-wasip2"),
    _ => None,
  }
}

/// Get the recommended entry point name pattern for a WASI component
pub fn get_wasi_component_entry_pattern() -> &'static str {
  "[name].component.wasm"
} 