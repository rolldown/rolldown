import './dep.js';
globalThis.events.push('target');
try {
  globalThis.read();
} catch (error) {
  globalThis.events.push(error.name);
}
export const x = 1;
