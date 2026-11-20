// An eager import with no interop: it runs `x.cjs` and needs no `__toESM`.
import './x.cjs';

export const name = 'eager';
