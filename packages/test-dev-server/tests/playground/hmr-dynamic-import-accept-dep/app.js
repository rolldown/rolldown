// `app` reaches `foo` ONLY through a dynamic import() and accepts it as a dep. Editing
// `foo` must bubble across the dynamic edge and stop at this edge boundary (app re-runs
// nothing; the accept callback fires with the fresh `foo`), not full-reload.
import('./foo.js').then((mod) => {
  document.querySelector('.foo').textContent = mod.value;
});

import.meta.hot.accept('./foo.js', (mod) => {
  document.querySelector('.foo').textContent = mod.value;
});
