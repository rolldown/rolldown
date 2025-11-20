export function variableBetweenSlash(dir) {
  return new URL(`./foo/${dir}/index.js`, import.meta.url);
}

export function variableBeforeNonSlash(dir) {
  return new URL(`./foo/${dir}.js`, import.meta.url);
}

export function twoVariables(dir, file) {
  return new URL(`./foo/${dir}${file}.js`, import.meta.url);
}

export function twoVariablesBetweenSlash(dir, dir2) {
  return new URL(`./foo/${dir}${dir2}/index.js`, import.meta.url);
}

export function ignoreStartingWithAVariable(file) {
  return new URL(`${file}.js`, import.meta.url);
}

export function viteIgnore(dir) {
  return new URL(/* @vite-ignore */ `./foo/${dir}.js`, import.meta.url);
}
