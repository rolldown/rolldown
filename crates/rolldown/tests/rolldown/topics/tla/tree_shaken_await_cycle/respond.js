import { clean } from './utils.js';
import { render_page } from './page.js';
(globalThis.tlaOrder ??= []).push('respond');
export function respond() {
  return clean(render_page());
}
