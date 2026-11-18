import { do_fetch } from './fetch.js';
import { clean } from './utils.js';
(globalThis.tlaOrder ??= []).push('page');
export function render_page() {
  return clean('page');
}
