import value from './dep.js';

// Accepts dep.js, so the edit does not re-run this module. Its factory still reads
// dep.js's interop, so the patch must carry it with a new stamp.
import.meta.hot.accept('./dep.js', () => {});

export const initial = value;
