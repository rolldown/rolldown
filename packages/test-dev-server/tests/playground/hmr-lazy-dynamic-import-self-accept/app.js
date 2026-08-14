// `app` reaches `foo` ONLY through a dynamic import() and self-accepts. Editing `foo`
// must bubble across the dynamic edge to this boundary and hot-update, not full-reload.
import.meta.hot?.accept();

import('./foo.js').then((mod) => {
  document.querySelector('.foo').textContent = mod.value;
});
