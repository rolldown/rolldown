// ESM consumer; imports ONLY the unrelated util, never the re-exported schemas.
import { objectKeys } from './lib/index.js';

export const run = () => (objectKeys ? 'ok' : 'no');
