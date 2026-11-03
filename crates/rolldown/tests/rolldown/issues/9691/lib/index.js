// Barrel with two `export *`. Resolving a name not directly exported here forces
// a star-search across every source (handler-a AND handler-b).
export * from './handler-a.js';
export * from './handler-b.js';

// A local export used by the dynamic-import trigger.
export const TAG = 'lib';
