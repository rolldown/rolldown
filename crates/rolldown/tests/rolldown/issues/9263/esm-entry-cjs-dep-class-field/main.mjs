// ESM entry — mirrors the React.lazy / route-chunk shape from the linked repro
// where a CJS-classified module is reached only through ESM boundaries.
// The exported binding keeps the constructor call from being tree-shaken away.
import Counter from './dep.cjs';

const c = new Counter();
c.tick();

export const ok = c.count === 1;
