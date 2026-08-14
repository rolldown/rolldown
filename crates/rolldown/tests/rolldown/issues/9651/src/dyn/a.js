// dynamic-import target; must be a separate module from the one that imports
// `../zod/external.js` (b.js) — merging them does not reproduce.
import './b.js';
