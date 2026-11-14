// `a` has no `await` of its own. It only reaches a module with top-level await.
import './tla.js';
import { b } from './b.js';

export function a() {
  return b();
}
