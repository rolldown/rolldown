import json from 'ext' assert { type: 'json' };

const options = { with: { kind: 'x' } };
const key = 'kind';

export const imports = [
  import('./dep.js', options),
  import('./dep.js', { with: options.with }),
  import('./dep.js', { with: { kind: 'x', [key]: 'y', size: 1 } }),
  import('./dep.js', { assert: { kind: 'x' } }),
];

export { json };
