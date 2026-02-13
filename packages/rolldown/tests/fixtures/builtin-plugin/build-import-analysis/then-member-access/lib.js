export const foo = 100;
export const bar = 200;
export const nested = { value: 300 };
export const unused = 999; // This should be tree-shaken
export default foo;
