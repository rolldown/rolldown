// `acceptExports(names, cb)` — on the client this behaves as a self-accept (the export
// names are a server-side concern). Editing this module runs the callback with the fresh
// module instead of full-reloading.
export const value = 'exports-v1';
document.querySelector('.value').textContent = value;

import.meta.hot?.acceptExports(['value'], (mod) => {
  document.querySelector('.value').textContent = mod.value;
});
