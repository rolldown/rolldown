export const lone = 'lone-v1';

document.querySelector('.lone').textContent = lone;

// Self-accepts + invalidates, but nothing above accepts it (main.js has no
// accept): the invalidate re-walk finds no boundary -> clean full reload.
import.meta.hot?.accept(() => {
  import.meta.hot.invalidate();
});
