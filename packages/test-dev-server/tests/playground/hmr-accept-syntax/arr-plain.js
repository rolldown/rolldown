// Plain array accept-dep: `import.meta.hot.accept(['./dep'], cb)`.
import { value } from './arr-plain-target.js';
document.querySelector('.arr-plain').textContent = value;

import.meta.hot.accept(['./arr-plain-target.js'], ([mod]) => {
  document.querySelector('.arr-plain').textContent = mod.value;
});
