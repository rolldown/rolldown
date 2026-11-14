// With `treeshake.commonjs: false`, `export *` of a CommonJS module uses the namespace object of
// this module, and the namespace object keeps `enable`.
export let flag = false;

export function enable() {
  flag = true;
}

export * from './dep.cjs';
