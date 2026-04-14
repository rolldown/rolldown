import { value } from './deep.js';

globalThis.__bug2_ready = value === 'hello';

export const unused = value;
