export const value = 'exports-v1';
export const other = 'other';
document.querySelector('.value').textContent = value;

import.meta.hot?.acceptExports(['value'], (mod) => {
  document.querySelector('.value').textContent = mod.value;
});
