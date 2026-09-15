// `app` reaches `foo` only through a dynamic `import()`, and self-accepts. Editing
// `foo` must bubble across the dynamic edge to this boundary (patch), not full-reload.
import.meta.hot.accept();

import('./foo.js').then((mod) => {
  console.log('.app', mod.value);
});
