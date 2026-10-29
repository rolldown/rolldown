import { childValue } from './child.js';

document.querySelector('.value').textContent = childValue;
window.__acceptCount = 0;

import.meta.hot.accept('./child.js', (mod) => {
  window.__acceptCount += 1;
  document.querySelector('.value').textContent = mod.childValue;
});
