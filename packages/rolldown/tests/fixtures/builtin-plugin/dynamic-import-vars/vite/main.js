export function basic(base) {
  return import(`./mods/${base}.js`)
}

// export function aliasPath(base) {
//   return import(`@/${base}.js`)
// }

// export function aliasPathWithMultiParentDir() {
//   return import(`#/${base}.js`)
// }

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


// Port from https://github.com/vitejs/vite/blob/main/packages/vite/src/node/__tests__/plugins/dynamicImportVar/__snapshots__/parse.spec.ts.snap

// exports[`parse positives > ? in url 1`] = `"__variableDynamicImportRuntimeHelper((import.meta.glob("./mo\\\\?ds/*.js", {"query":"?url","import":"*"})), \`./mo?ds/\${base ?? foo}.js\`)"`;

// exports[`parse positives > ? in variables 1`] = `"__variableDynamicImportRuntimeHelper((import.meta.glob("./mods/*.js", {"query":"?raw","import":"*"})), \`./mods/\${base ?? foo}.js\`)"`;

// exports[`parse positives > ? in worker 1`] = `"__variableDynamicImportRuntimeHelper((import.meta.glob("./mo\\\\?ds/*.js", {"query":"?worker","import":"*"})), \`./mo?ds/\${base ?? foo}.js\`)"`;

// exports[`parse positives > alias path 1`] = `"__variableDynamicImportRuntimeHelper((import.meta.glob("./mods/*.js")), \`./mods/\${base}.js\`)"`;

// exports[`parse positives > alias path with multi ../ 1`] = `"__variableDynamicImportRuntimeHelper((import.meta.glob("../../*.js")), \`../../\${base}.js\`)"`;

// exports[`parse positives > basic 1`] = `"__variableDynamicImportRuntimeHelper((import.meta.glob("./mods/*.js")), \`./mods/\${base}.js\`)"`;

// exports[`parse positives > with ../ and itself 1`] = `"__variableDynamicImportRuntimeHelper((import.meta.glob("../dynamicImportVar/*.js")), \`./\${name}.js\`)"`;

// exports[`parse positives > with multi ../ and itself 1`] = `"__variableDynamicImportRuntimeHelper((import.meta.glob("../../plugins/dynamicImportVar/*.js")), \`./\${name}.js\`)"`;

// exports[`parse positives > with query 1`] = `"__variableDynamicImportRuntimeHelper((import.meta.glob("./mods/*.js", {"query":"?foo=bar"})), \`./mods/\${base}.js\`)"`;

// exports[`parse positives > with query raw 1`] = `"__variableDynamicImportRuntimeHelper((import.meta.glob("./mods/*.js", {"query":"?raw","import":"*"})), \`./mods/\${base}.js\`)"`;

// exports[`parse positives > with query url 1`] = `"__variableDynamicImportRuntimeHelper((import.meta.glob("./mods/*.js", {"query":"?url","import":"*"})), \`./mods/\${base}.js\`)"`;