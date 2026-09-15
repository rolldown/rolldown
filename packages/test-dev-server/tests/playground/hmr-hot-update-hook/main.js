import { depValue } from './dep.js';

document.querySelector('.value').textContent = depValue;
window.__acceptCount = 0;

import.meta.hot.accept('./dep.js', (mod) => {
  window.__acceptCount += 1;
  document.querySelector('.value').textContent = mod.depValue;
});
