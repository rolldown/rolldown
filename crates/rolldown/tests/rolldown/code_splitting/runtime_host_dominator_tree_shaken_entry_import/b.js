export { value } from './shared.js';
export { keys, runBackend } from './consumer.js';
export const lazyLog = () => import('./logger.js').then((m) => m.log('b'));
