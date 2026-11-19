import { clause, declared } from './b.js';

declared();
clause();

export const load = () => import('./lazy.js');
