export const condValue = 'cond-v1';

document.querySelector('.cond').textContent = condValue;

// A conditional accept whose condition was TRUE at execution time — the
// accept ran, so this module IS a runtime HMR boundary and hot-updates.
if (true) {
  import.meta.hot?.accept((mod) => {
    if (mod) {
      document.querySelector('.cond').textContent = mod.condValue;
    }
  });
}
