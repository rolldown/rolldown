export { value } from './a.mjs';
export function unused() {
  import('./a.mjs');
}
