export { task } from './task.js';
import { s_task as inner } from './helper.js';
export const useInner = () => inner;
export const load = () => [import('./seg_a.js'), import('./seg_b.js')];
