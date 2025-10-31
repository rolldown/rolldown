/**
 * This is a JSDoc comment example.
 * @param {string} name
 * @default 'world'
 */
export function hello(name) {
  // This is a regular comment
  return `Hello ${name}`;
}

/*! This is a legal comment with @license */
export const foo = 'bar';

/**
 * @preserve This should be preserved
 */
export const preserved = 'value';
