import { state } from './state.js';

export function getConfig(key, defaultValue) {
  // Accesses state directly (not state?.config) — this would throw if
  // state were undefined, providing a strong validation that strict
  // execution order wrapping prevents TDZ.
  const value = state.config?.[key];
  if (value !== undefined) return value;
  return defaultValue;
}

