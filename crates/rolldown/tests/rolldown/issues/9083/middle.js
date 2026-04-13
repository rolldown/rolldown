// Transitively depends on deep.js (no own TLA)
import { value } from './deep.js';
export const manager = { value, ready: false };
export function setup() {
  manager.ready = true;
}
