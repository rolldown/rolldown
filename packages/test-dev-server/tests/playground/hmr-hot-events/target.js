// Self-accepts, so editing it triggers a js-update — which fires the `vite:beforeUpdate`
// and `vite:afterUpdate` events registered in `listener.js`.
export const value = 'target-v1';
document.querySelector('.value').textContent = value;

import.meta.hot?.accept((mod) => {
  document.querySelector('.value').textContent = mod.value;
});
