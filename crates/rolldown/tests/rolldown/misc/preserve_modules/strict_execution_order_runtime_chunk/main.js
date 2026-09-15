import dep, { named } from './dep.js';

globalThis.__events.push('main ' + dep() + named);

const mod = await import('./lazy.js');

globalThis.__events.push('after ' + mod.lazy);
