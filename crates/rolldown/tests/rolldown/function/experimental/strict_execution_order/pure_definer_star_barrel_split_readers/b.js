import * as ns from './barrel.js';

(globalThis.__events ??= []).push('b');

// This entry reads only the SIBLING binding — the split read. With both entries reading the
// definer, on-demand wrapping stays green; the split is what makes on-demand drop `init_definer`
// from the barrel init as well, so the case fails in both wrap modes.
export const sibValue = ns.vSib;
