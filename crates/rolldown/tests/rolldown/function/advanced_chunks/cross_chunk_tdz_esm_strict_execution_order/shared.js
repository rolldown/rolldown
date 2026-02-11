import { getConfig } from './helpers.js';

// With strictExecutionOrder enabled, this module can be split into a separate
// chunk even if it creates circular chunk imports, because wrapped modules use
// lazy initializers and avoid TDZ.
export const TIMEOUT = getConfig('TIMEOUT', 300000);
export const MAX_RETRIES = getConfig('MAX_RETRIES', 3);

