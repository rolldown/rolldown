// ESM entry. `lib.cjs` must be a *dependency*, not the entry: as the entry its require-locals are
// dropped and nothing collides.
import lib from './lib.cjs';

export default lib;
