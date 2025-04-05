import {
  loadRemoteToRegistry,
  loadSharedToRegistry,
} from 'mf:remote-module-registry.js';

if (__IS__SHARED__) {
  await loadSharedToRegistry(__MODULE_ID__);
} else {
  await loadRemoteToRegistry(__MODULE_ID__);
}
