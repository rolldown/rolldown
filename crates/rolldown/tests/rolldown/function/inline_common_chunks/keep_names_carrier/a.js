import { Object as shared } from './shared.js';

const early = Object.name;
globalThis.Object.freeze(Object);
function Object() {}

console.log(early, shared.name);
