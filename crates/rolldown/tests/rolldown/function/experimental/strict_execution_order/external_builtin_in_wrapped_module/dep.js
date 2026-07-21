import { sep } from 'node:path';

globalThis.__depRan = true;

export const marker = `dep:${sep.length > 0}`;
