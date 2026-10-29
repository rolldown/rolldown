// Optional array accept-dep: `import.meta.hot?.accept(['./dep'], cb)`.
import { value } from './arr-optional-target.js';
document.querySelector('.arr-optional').textContent = value;

import.meta.hot?.accept(['./arr-optional-target.js'], ([mod]) => {
  document.querySelector('.arr-optional').textContent = mod.value;
});
