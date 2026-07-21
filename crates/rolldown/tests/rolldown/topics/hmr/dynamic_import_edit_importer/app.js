// Editing this self-accepting module alone produces a patch that carries only `app` —
// `foo` is not in the payload. The snapshot pins the registry-gated `import()` rewrite
// (`initModule` + `loadExports`): this factory stays resident across later patches, so
// it must be able to re-run `foo` after an eviction, not just read it from the cache.
import.meta.hot.accept();

import('./foo.js').then((mod) => {
  console.log('.app', mod.value);
});
