// A default import from CommonJS is read through the interop binding `import_dep`
// (`dep.v` prints as `import_dep.v`), so a parameter named `import_dep` must not capture it.
import dep from './dep.cjs';

export function read(import_dep) {
  return [dep.v, import_dep];
}
