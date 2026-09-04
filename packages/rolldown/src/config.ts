// This entry loads the binding (via `loadConfig`); every such entry registers
// the CurrentThread timer host at import (see timer-host.ts).
import './timer-host';

export { defineConfig } from './utils/define-config';
export { loadConfig } from './utils/load-config';
export { VERSION } from './constants/version';
