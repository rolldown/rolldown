import { baz } from './baz.js';

export const bar = `bar(${baz})`;

(window.__hotStates ??= []).push({
  id: 'bar',
  gen: import.meta.hot?.data.gen ?? null,
  own: import.meta.hot?.data.own ?? null,
});

if (import.meta.hot) {
  import.meta.hot.data.own = 'bar-own-write';
}

import.meta.hot?.dispose((data) => {
  (window.__disposed ??= []).push('bar');
  data.gen = (import.meta.hot.data.gen ?? 0) + 1;
});
