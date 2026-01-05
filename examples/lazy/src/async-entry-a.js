import asyncLibA from './async-lib-a.js';

console.log('async-entry-a.js', asyncLibA);
document.getElementById('root').innerHTML += 'async-entry-a.js loaded\n';

export const isAsyncEntry = true;
