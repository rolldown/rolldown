globalThis.sideEffect = {};
globalThis.events.push('dep-a');
module.exports = globalThis.sideEffect;
