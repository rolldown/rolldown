import './a.js';

globalThis.readValue = () => 42;

export const declared = /* @__NO_SIDE_EFFECTS__ */ () => globalThis.readValue();
const clause = /* @__NO_SIDE_EFFECTS__ */ () => globalThis.readValue();

export { clause };
