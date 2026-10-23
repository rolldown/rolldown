import { ok } from 'node:assert/strict';

ok(true);
export { value as mainValue } from './shared.js';
export function loadDynamic() {
  return import('./dynamic.js');
}
