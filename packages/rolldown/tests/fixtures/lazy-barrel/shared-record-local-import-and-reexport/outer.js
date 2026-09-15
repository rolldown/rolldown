// Outer barrel. One import record carries BOTH `setup` (used by the local
// `build` export) and `helper` (re-exported below) -- a "mixed" record.
import { setup, helper } from './inner.js';

export function build() {
  return setup();
}

export { helper };
