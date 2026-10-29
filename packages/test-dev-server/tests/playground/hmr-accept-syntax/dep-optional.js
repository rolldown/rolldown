// Optional accept-dep: `import.meta.hot?.accept('./dep', cb)`.
import { value } from './dep-optional-target.js';
document.querySelector('.dep-optional').textContent = value;

import.meta.hot?.accept('./dep-optional-target.js', (mod) => {
  document.querySelector('.dep-optional').textContent = mod.value;
});
