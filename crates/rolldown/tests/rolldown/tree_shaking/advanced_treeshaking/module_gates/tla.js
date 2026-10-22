// The analysis covers a dependency with top-level await.
await 1;
const o = { used: 1, dead: 'DEAD_dependency_tla' };
export const value = o.used;
