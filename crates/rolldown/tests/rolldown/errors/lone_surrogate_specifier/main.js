// Every form reports a module specifier with a lone surrogate.
import a from './a\uD800.js';
export * from './b\uD800.js';
export { c } from './c\uD800.js';
import('./d\uD800.js');
require(`./e\uD800.js`);
try {
  require('./k\uD800.js');
} catch {}
new URL('./f\uD800.png', import.meta.url);

// Attribute keys and values are reported too.
import './g.js' with { type: 'json\uD800' };
import './h.js' with { 'type\uD800': 'json' };

// An ignored specifier and a data URL are left untouched.
import(/* @vite-ignore */ './i\uD800.js');
new URL(/* @vite-ignore */ './j\uD800.png', import.meta.url);
new URL('data:text/plain,\uD800', import.meta.url);

export { a };
