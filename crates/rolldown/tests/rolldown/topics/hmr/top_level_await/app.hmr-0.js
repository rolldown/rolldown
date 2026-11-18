// The factory that wraps this module in an HMR patch must be `async`, or `await`
// is a syntax error.
import.meta.hot.accept();

const value = await Promise.resolve(2);
console.log(value);
