import { state } from './state.js';

export function getConfig(key, defaultValue) {
  // Accesses state directly (not state?.config) — this would throw if
  // state were undefined due to TDZ, providing a stronger validation
  // that cycle prevention keeps everything in one chunk.
  const value = state.config?.[key];
  if (value !== undefined) return value;
  return defaultValue;
}
