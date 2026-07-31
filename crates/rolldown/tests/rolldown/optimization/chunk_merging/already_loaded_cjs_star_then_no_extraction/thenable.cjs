// Rolldown marks this as a module with `has_dynamic_exports`, meaning
// that it does not know statically whether there is a `then` export
// or not.
const key = 'then';
exports[key] = function (resolve) {
  resolve('intercepted');
};
