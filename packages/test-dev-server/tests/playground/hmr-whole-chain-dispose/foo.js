import { bar } from './bar.js';

document.querySelector('.chain').textContent = bar;

// Record what this generation sees in its `hot.data` bag AT EXECUTION time.
// `import.meta.hot.data` is PRESERVED across a module's HMR generations (the
// bag is the same object), so both the disposer's write (`gen`) and this
// generation's own direct write (`own`) are visible to the next generation.
(window.__hotStates ??= []).push({
  id: 'foo',
  gen: import.meta.hot?.data.gen ?? null,
  own: import.meta.hot?.data.own ?? null,
});

// A direct write into the live bag — because the bag persists, the NEXT
// generation must read this value back (not a fresh, empty bag).
if (import.meta.hot) {
  import.meta.hot.data.own = 'foo-own-write';
}

import.meta.hot?.dispose((data) => {
  (window.__disposed ??= []).push('foo');
  // `data` is the same preserved bag as `import.meta.hot.data`; bump the
  // generation counter that the next execution reads back.
  data.gen = (import.meta.hot.data.gen ?? 0) + 1;
});

import.meta.hot?.accept();
