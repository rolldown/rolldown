export { then as keepThen } from './target.js';
import('./dropped.js');
import('./dropped.cjs');
import('./kept/kept.js');
export const load = () => import('./target.js').then((ns) => ns.value);
export const loadInFunction = () => import('./in_function.js').then(() => 'loaded');
