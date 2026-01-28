/* oxlint-disable */

import assert from 'node:assert';
import nodeFs from 'node:fs';

// INJECT_CONFIG_HERE

console.log('Config loaded:', CONFIG);
assert(CONFIG, 'Config should be defined');
assert.equal(CONFIG.message, 'hello world');

if (import.meta.hot) {
  import.meta.hot.accept((newModule) => {
    console.log('HMR triggered, new config:', newModule.CONFIG);
    // Verify the config was updated
    assert(newModule.CONFIG.updated, 'Config should be updated');
    assert.equal(newModule.CONFIG.version, 2, 'Config version should be 2');
    // Write a marker file to indicate HMR worked
    nodeFs.writeFileSync('./ok-0', 'transform dependency HMR worked');
  });
}

export { CONFIG }; // oxlint-disable-line
