// This leaf has a later exec_order than a.js.
// moduleId prints it before a.js.
const x = ['b'];
export function b() {
  return x[0];
}
