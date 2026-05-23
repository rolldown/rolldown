import './shared-user.js';

const dynamicRoot = await import('./dynamic-entry.js');

export const result = dynamicRoot.value;
