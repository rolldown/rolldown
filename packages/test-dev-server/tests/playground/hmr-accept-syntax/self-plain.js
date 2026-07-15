// Plain self-accept: `import.meta.hot.accept(cb)` (no optional chaining).
export const value = 'self-plain-v1';
document.querySelector('.self-plain').textContent = value;

import.meta.hot.accept((mod) => {
  document.querySelector('.self-plain').textContent = mod.value;
});
