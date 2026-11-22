import { join } from 'node:path';

globalThis.__extLog.push('d');
export const x = join('a', 'b');
