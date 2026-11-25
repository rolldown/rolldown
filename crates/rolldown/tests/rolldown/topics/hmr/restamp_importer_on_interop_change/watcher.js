import value from './dep.js';

// Accepts dep.js, so the edit does not re-run this module. Its factory still reads
// dep.js's interop, so the push marks it stale too: an update that re-runs it later pulls
// the new factory.
import.meta.hot.accept('./dep.js', () => {});

export const initial = value;
