export function f(name) {
  return import(`./${'sub'}/views/${name}.js`);
}
