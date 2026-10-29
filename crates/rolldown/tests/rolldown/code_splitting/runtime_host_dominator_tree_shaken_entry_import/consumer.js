import backend from 'server-only';
import * as ns from './nsB.js';
globalThis.__consumer_ran = true;
export const keys = Object.keys(ns);
export function runBackend() {
  return backend();
}
