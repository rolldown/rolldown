import { bar } from './bar.js';

document.querySelector('.chain').textContent = bar;

// Record what this generation sees in its `hot.data` bag AT EXECUTION time.
(window.__hotStates ??= []).push({
  id: 'foo',
  gen: import.meta.hot?.data.gen ?? null,
  leak: import.meta.hot?.data.leak ?? null,
});

// An own write into the live bag — a FRESH bag per generation means this must
// NOT be visible to the next generation.
if (import.meta.hot) {
  import.meta.hot.data.leak = 'foo-own-write';
}

import.meta.hot?.dispose((data) => {
  (window.__disposed ??= []).push('foo');
  // Read the OLD generation's bag via hot.data, write into the FRESH bag.
  data.gen = (import.meta.hot.data.gen ?? 0) + 1;
});

import.meta.hot?.accept();
