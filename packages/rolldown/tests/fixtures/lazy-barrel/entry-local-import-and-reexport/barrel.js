import { x } from './m.js'; // record#0: imported for local use (in useX)
export { y } from './m.js'; // re-export of a different symbol
export function useX() {
  return x();
}
