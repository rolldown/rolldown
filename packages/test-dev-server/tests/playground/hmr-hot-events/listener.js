// Registers `import.meta.hot.on(...)` for the built-in HMR events. This module is never
// edited, so its listeners persist across updates to `target.js`.
const log = [];
const render = () => {
  document.querySelector('.events').textContent = log.join(',');
};
render();

import.meta.hot?.on('vite:beforeUpdate', () => {
  log.push('before');
  render();
});
import.meta.hot?.on('vite:afterUpdate', () => {
  log.push('after');
  render();
});
