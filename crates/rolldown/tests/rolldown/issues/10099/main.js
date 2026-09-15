// Consumer imports ONLY the external passthrough re-export (real-world:
// `import { dayjs } from 'element-plus'`). Because `extdep`'s canonical is an
// external module's default that is re-exported by the barrel, resolving it
// force-includes the barrel `index.js` -- whose unrelated top-level
// `defaults_default.install/.version` reads must NOT be retained once
// `defaults_default`'s import and definition are tree-shaken away (#10099).
import { extdep } from './index.js';

if (!extdep || extdep.name !== 'extdep-default') {
  throw new Error('external passthrough did not resolve');
}

console.log('ok:', extdep.name);
