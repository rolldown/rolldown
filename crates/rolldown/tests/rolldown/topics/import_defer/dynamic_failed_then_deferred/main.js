let err1;
await import('./throws.js').catch((e) => {
  err1 = e;
});
const { ns } = await import('./import-defer-throws.js');
let err2;
try {
  ns.foo;
} catch (e) {
  err2 = e;
}
// A deferred access to a module that failed earlier throws the same error.
export const result = [err1?.someError, err1 === err2];
