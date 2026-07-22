// Uses names living in the 1st and 3rd star targets; b.js is probed but its
// export is unused, d.js is never even probed.
import { fromA, fromC } from './barrel.js';
export const result = fromA + '|' + fromC;
