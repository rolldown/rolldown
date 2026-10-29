import { a } from './a.js';
import { b } from './b.js';

export const combined = a + b;

globalThis.__mixed_runs_hub = (globalThis.__mixed_runs_hub ?? 0) + 1;

if (import.meta.hot) {
  import.meta.hot.accept(() => {});
}
