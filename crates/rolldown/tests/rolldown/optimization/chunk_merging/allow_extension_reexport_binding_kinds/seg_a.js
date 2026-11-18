export { task as s_task } from './task.js';
export { foo as s_foo } from './dep.cjs';
export { sep as s_sep } from 'node:path';
export * as ns from 'node:path';
const v = () => 'v';
export { v, v as w };
export const x = () => 'x';
