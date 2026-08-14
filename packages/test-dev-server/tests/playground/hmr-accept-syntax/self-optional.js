// Optional self-accept: `import.meta.hot?.accept(cb)`.
export const value = 'self-optional-v1';
document.querySelector('.self-optional').textContent = value;

import.meta.hot?.accept((mod) => {
  document.querySelector('.self-optional').textContent = mod.value;
});
