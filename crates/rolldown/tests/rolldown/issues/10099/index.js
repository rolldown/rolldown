// Mirrors element-plus/es/index.mjs (the barrel). It default-imports `defaults.js`
// and reads properties off it at the top level, AND re-exports an EXTERNAL
// module's default (`external_default`, real-world `dayjs`) as a passthrough.
import defaults_default from './defaults.js';
import external_default from 'extdep';

const install = defaults_default.install;
const version = defaults_default.version;
var barrel_default = defaults_default;

// `extdep` is the only export the entry uses; it is a passthrough of an external
// module's default, which is what forces this barrel to be included.
export { external_default as extdep, install, version, barrel_default as default };
