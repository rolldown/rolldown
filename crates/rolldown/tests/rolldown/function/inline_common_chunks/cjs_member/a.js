import cjs, { bump } from './cjs.js';
globalThis.events.push('A ' + bump() + ' ' + (cjs.self() === cjs));
globalThis.markers.push(cjs);
