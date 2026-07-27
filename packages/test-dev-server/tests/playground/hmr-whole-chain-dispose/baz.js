export const baz = 'baz-v1';

(window.__hotStates ??= []).push({
  id: 'baz',
  gen: import.meta.hot?.data.gen ?? null,
  own: import.meta.hot?.data.own ?? null,
});

if (import.meta.hot) {
  import.meta.hot.data.own = 'baz-own-write';
}

import.meta.hot?.dispose((data) => {
  (window.__disposed ??= []).push('baz');
  data.gen = (import.meta.hot.data.gen ?? 0) + 1;
});
