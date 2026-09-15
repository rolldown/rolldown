// `foo` and `bar` are edited in the SAME watch batch. The debounced watcher must fold
// both changes into ONE hot update, so this accept callback fires once with both new
// values — never twice with a half-applied pair.
import { value as barValue } from './bar.js';
import { value as fooValue } from './foo.js';

document.querySelector('.foo').textContent = fooValue;
document.querySelector('.bar').textContent = barValue;

import.meta.hot?.accept((mod) => {
  (window.__updates ??= []).push({ foo: mod.fooValue, bar: mod.barValue });
});

export { barValue, fooValue };
