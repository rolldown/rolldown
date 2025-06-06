/**
 * Mock implementation of the wasi_features.rs module from Rust
 */
import { Platform } from './inner_bundler_options/types/platform.js';

/**
 * Checks if the given platform is any version of WASI
 */
export function is_wasi_platform(platform) {
  return platform === Platform.Wasi || platform === Platform.WasiP2;
}

/**
 * Checks if the platform is specifically WASI Preview 2
 */
export function is_wasi_preview2(platform) {
  return platform === Platform.WasiP2;
}

/**
 * Returns the appropriate target triple for the given WASI platform
 */
export function get_wasi_target_triple(platform) {
  switch (platform) {
    case Platform.Wasi: return "wasm32-wasip1-threads";
    case Platform.WasiP2: return "wasm32-wasip2";
    default: return null;
  }
}

/**
 * Get the recommended entry point name pattern for a WASI component
 */
export function get_wasi_component_entry_pattern() {
  return "[name].component.wasm";
} 