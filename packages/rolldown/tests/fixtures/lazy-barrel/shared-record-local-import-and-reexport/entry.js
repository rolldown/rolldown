import { build, helper } from './outer.js'; // requests a local export + a re-export together

// `build()` runs at module-eval time. With the bug, `store` is dropped from the
// bundle, so importing this entry throws `ReferenceError: store is not defined`.
export const result = [build(), helper()];
