// The edit adds a NEW static edge to heavy.js — already in the graph through the
// never-triggered import() and never executed by this client.
import { heavy } from './heavy.js';

export const value = 'v2';
console.log('hmr', value, heavy);
import.meta.hot.accept();
