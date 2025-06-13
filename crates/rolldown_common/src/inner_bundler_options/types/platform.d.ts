/**
 * Platform enum representing different target platforms for Rolldown
 */
export enum Platform {
  Node = 0,
  Browser = 1,
  Neutral = 2,
  Wasi = 3,
  WasiP2 = 4,
}

/**
 * Convert a platform string to the corresponding Platform enum value
 * @param platform Platform string (e.g., 'node', 'browser', 'wasi', 'wasip1', 'wasip2')
 * @returns Platform enum value
 * @throws Error if the platform string is not supported
 */
export function tryFrom(platform: string): Platform;
