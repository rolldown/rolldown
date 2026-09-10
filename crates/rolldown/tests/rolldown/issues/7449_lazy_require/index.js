import { load } from './lazy.cjs';
import './b.js';

if (globalThis.aRan) {
  throw new Error('a.cjs must not run before load() is called');
}
load();
if (!globalThis.aRan) {
  throw new Error('a.cjs must run when load() is called');
}
