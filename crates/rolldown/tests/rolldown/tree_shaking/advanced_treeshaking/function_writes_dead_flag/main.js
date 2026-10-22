let DEAD_initialized = false;
function initFeatureFlags() {
  if (DEAD_initialized) return;
  DEAD_initialized = true;
}
export function createApp() {
  initFeatureFlags();
  return { mount() {} };
}
