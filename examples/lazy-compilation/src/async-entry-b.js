import asyncLibB from './async-lib-b.js';
import './async-lib-shared.js';

console.log('async-entry-b.js', asyncLibB);
document.getElementById('root').innerHTML += '[async-entry-b.js] loaded\n';
