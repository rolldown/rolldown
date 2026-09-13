import { B } from './b.js';

// Module-init (top-level) use of a cyclic import. In full-bundle-mode this
// reference is emitted unbound -> `ReferenceError: B is not defined`.
// See https://github.com/rolldown/rolldown/issues/9946.
export const defaults = { button: B };
