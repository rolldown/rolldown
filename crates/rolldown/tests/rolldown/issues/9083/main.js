// Imports from barrel which re-exports from middle which transitively depends on deep (TLA)
import { manager, setup } from './barrel.js';
setup();
export { manager };
