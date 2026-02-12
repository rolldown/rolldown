/* oxlint-disable */

import assert from 'node:assert';
import nodeFs from 'node:fs';

// INJECT_CONFIG_HERE

console.log('Config loaded:', CONFIG);
assert(CONFIG, 'Config should be defined');
assert.equal(CONFIG.message, 'hello world');

const reloadHappened = nodeFs.existsSync('./ok-0');
if (reloadHappened) {
  nodeFs.writeFileSync('./ok-1', 'reload triggered');
}

if (import.meta.hot) {
  import.meta.hot.accept((newModule) => {
    console.log('HMR triggered, new config:', newModule.CONFIG);
    const version = newModule.CONFIG.version;

    // Determine which step we're on based on whether ok-0 already exists
    const isAfterReload = nodeFs.existsSync('./ok-0');

    if (isAfterReload) {
      // Step 2: after reload, config should be version 3
      assert.equal(version, 3, `After reload, config version should be 3, got ${version}`);
      nodeFs.writeFileSync('./ok-2', 'transform dependency HMR worked after reload');
    } else {
      // Step 0: before reload, config should be version 2
      assert.equal(version, 2, `Before reload, config version should be 2, got ${version}`);
      nodeFs.writeFileSync('./ok-0', 'transform dependency HMR worked');
    }
  });
}

export { CONFIG }; // oxlint-disable-line
