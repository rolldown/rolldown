export function basic(base) {
  return import(`./mods/${base}.js`)
}

export function aliasPath(base) {
  return import(`@/${base}.js`)
}

export function aliasPathWithMultiParentDir(base) {
  return import(`#/${base}.js`)
}

export function withQuery(base) {
  return import(`./mods/${base}.js?foo=bar`)
}

export function withQueryRaw(base) {
  return import(`./mods/${base}.js?raw`)
}

export function withQueryUrl(base) {
  return import(`./mods/${base}.js?url`)
}

export function wildcardInVariables(base) {
  return import(`./mods/${base ?? foo}.js?raw`)
}

export function wildcardInUrl(base) {
  // The `?` is not escaped on windows 
  // (`?` cannot be used as a filename on windows)
  return import(`./mods/${base ?? foo}.js?raw`)
}

export function wildcardInWorker(base) {
  // The `?` is not escaped on windows 
  // (`?` cannot be used as a filename on windows)
  return import(`./mo?ds/${base ?? foo}.js?worker`)
}

export function withParentDirAndItself(name) {
  return import(`../dynamicImportVar/${name}.js`)
}

export function withMultiParentDirAndItself(name) {
  return import(`../../plugins/dynamicImportVar/${name}.js`)
}