// `shim.mjs` is discovered before `main.js`, so its binding is linked to the
// external namespace first. Linking `main.js` afterwards then resolves through
// the already-compressed parent.
import './shim.mjs';

export { readDefault } from './main.js';
