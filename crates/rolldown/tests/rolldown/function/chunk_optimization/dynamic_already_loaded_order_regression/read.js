import './base.js';

if (!globalThis.ready) throw new Error('read.js ran before setup.js');

export function readValue() {
  return 'read';
}
