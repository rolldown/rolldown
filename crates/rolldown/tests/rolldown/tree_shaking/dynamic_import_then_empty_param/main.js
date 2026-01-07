// These two should have the same tree shaking behavior
// Both should tree shake all exports from lib.js

// Empty parameter list - currently bails out (incorrect)
import('./lib.js').then(() => {});

// Empty destructured object - currently tree shakes correctly
import('./lib.js').then(({}) => {});
