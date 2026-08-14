import { sharedB } from './common-b.js';

console.log('entry-a');

globalThis.loadDynamic1FromEntryA = () => import('./dynamic1.js');
globalThis.loadDynamic2FromEntryA = () => import('./dynamic2.js');
