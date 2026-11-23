import './ready.js';
import './b.js';

await import('./b.js');
globalThis.calls.push('main done');
