import './button.css.js';
export const SIZE = 'md';
const state = (globalThis.state ??= {});
state.ready = true;
if (!globalThis.state.ready) throw new Error('button.js ran only in part');
