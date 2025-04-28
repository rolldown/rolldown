export function dynamicImport(name) {
  return import(`./mod/${name}.js`)
}
