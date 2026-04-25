import { marker } from './dep.js';

const state = { config: { REQUEST_TIMEOUT_MS: '300000', marker } };

export function getEnvString(key) {
  return state.config?.[key];
}

export function getEnvInt(key) {
  return Number(getEnvString(key) || 300000);
}
