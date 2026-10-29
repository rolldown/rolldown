/* oxlint-disable */
// The `transform` plugin in dev.config.mjs replaces the marker below with
// `const CONFIG = {...}` from `config.json` and watches that file.
// INJECT_CONFIG_HERE

document.querySelector('.config').textContent = `${CONFIG.message} v${CONFIG.version}`;

import.meta.hot?.accept();
