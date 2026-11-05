// The `import()` below makes `Promise` a name this module's body prints, so the parameter
// `Promise` is renamed. The new name must not be `Promise$1`: that is a root-scope local of this
// module, which kept its original name and is invisible to the conflict resolver. Landing on it
// silently makes the function read its own parameter instead of the local.
const Promise$1 = { tag: 'root-local' };

function read(Promise) {
  return [Promise.tag, Promise$1.tag];
}

module.exports = { read, load: () => import('./dep.js') };
