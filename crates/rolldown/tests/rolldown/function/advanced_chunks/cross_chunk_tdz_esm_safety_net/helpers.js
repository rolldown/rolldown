import { state } from './state.js';

export function getConfig(key, defaultValue) {
  // IMPORTANT: access via optional chaining so that when state is hoisted-undefined
  // (after const->var conversion), we get undefined instead of throwing.
  const value = state?.config?.[key];
  if (value !== undefined) return value;
  return defaultValue;
}

