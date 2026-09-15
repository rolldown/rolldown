export const value = 'off-v1';
document.querySelector('.value').textContent = value;

import.meta.hot?.accept((mod) => {
  document.querySelector('.value').textContent = mod.value;
});
