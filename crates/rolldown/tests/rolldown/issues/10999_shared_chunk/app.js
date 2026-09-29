import { h1 } from './h1.js';
import info from './info.json' with { type: 'json' };

export function run() {
  const m = require('./lazy.js');
  return [h1(), info.name, m.default()];
}

export function useApp() {
  return 'app';
}
