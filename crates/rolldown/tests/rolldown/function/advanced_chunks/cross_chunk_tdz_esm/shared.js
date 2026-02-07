import { getConfig } from './helpers.js';

// Cycle prevention detects that splitting this module into a separate chunk
// would create a circular chunk dependency, so it stays in the entry chunk.
// All const bindings are preserved (not converted to var).
export const TIMEOUT = getConfig('TIMEOUT', 300000);
export const MAX_RETRIES = getConfig('MAX_RETRIES', 3);
