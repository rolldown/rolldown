// The export code prints `Object.defineProperty(exports, Symbol.toStringTag, ...)` at the chunk's
// top level, before this module's code. The top-level `Symbol` must not capture it.
const Symbol = () => 'local';
export const s = Symbol();
