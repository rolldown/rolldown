// `node-mode.mjs` imports `node:https` in node mode and `non-node.js` imports it in non-node mode.
// So the chunk needs a second namespace for `node:https`, and rolldown makes its name.
export { read } from './node-mode.mjs';
export { value } from './non-node.js';
