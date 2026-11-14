// Both parents sit behind dynamic imports, so both are in the module graph — but only
// `parent-executed` ever runs in this tab.
window.__executed = [];

const load = (id) => {
  if (id === 'executed') {
    return import('./parent-executed.js');
  }
  return import('./parent-cold.js');
};

load('executed');
