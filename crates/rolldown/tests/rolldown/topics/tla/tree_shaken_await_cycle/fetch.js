import { respond } from './respond.js';
(globalThis.tlaOrder ??= []).push('fetch');
export function do_fetch() {
  return respond();
}
