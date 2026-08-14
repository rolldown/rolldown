// Plain accept-dep: `import.meta.hot.accept('./dep', cb)`.
import { value } from './dep-plain-target.js';
document.querySelector('.dep-plain').textContent = value;

import.meta.hot.accept('./dep-plain-target.js', (mod) => {
  document.querySelector('.dep-plain').textContent = mod.value;
});
