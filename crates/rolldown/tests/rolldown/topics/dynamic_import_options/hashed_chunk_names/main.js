export const loaded = import('./target.js', {}).then((target) => target.value);
