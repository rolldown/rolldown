// `app` reaches `foo` only through a dynamic `import()`, and accepts it as a dep.
// Editing `foo` must bubble across the dynamic edge and stop at this edge boundary
// (patch: boundary `app.js`, acceptedVia `foo.js`), not full-reload.
import('./foo.js').then((mod) => {
  console.log('.app', mod.value);
});

import.meta.hot.accept('./foo.js', (mod) => {
  console.log('.app', mod.value);
});
