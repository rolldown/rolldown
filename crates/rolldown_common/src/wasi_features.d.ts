import { Platform } from './inner_bundler_options/types/platform';

/**
 * Check if the given platform is a WASI platform (either Preview 1 or Preview 2)
 * @param platform Platform to check
 * @returns true if the platform is WASI (Preview 1 or 2), false otherwise
 */
export function is_wasi_platform(platform: Platform): boolean;

/**
 * Check if the given platform is specifically WASI Preview 2
 * @param platform Platform to check
 * @returns true if the platform is WASI Preview 2, false otherwise
 */
export function is_wasi_preview2(platform: Platform): boolean;

/**
 * Get the appropriate target triple for the given WASI platform
 * @param platform WASI platform
 * @returns Target triple string (e.g., 'wasm32-wasip1-threads', 'wasm32-wasip2') or null if not a WASI platform
 */
export function get_wasi_target_triple(platform: Platform): string | null;
