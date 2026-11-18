import './a.js';

const local = /* #__NO_SIDE_EFFECTS__ */ () => {};
function hoisted() {}

export { local, hoisted };
export { reexported } from './b.js';
