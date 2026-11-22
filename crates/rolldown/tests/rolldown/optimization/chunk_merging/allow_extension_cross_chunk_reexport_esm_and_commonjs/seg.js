export { task as s_task } from './task.js';
export { foo as s_foo } from './dep.cjs';
const v = () => 'v';
export { v, v as w };
export const x = () => 'x';
