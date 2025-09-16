export function singleDir(name) {
  return import(`./dir/a/${name}.js`)
}

export function multiDirs(dir, name) {
  return import(`./dir/${dir}/${name}.js`)
}

export function noFile(name) {
  return import(/* hello */`./dir/c/${name}.js`)
}

export function withAlias(name) {
  return import(/** @vite-ignore */ `@/${name}.js`)
}

export function withIgnoreTag(name) {
  return import(/* @vite-ignore */ `./dir/a/${name}.js`)
}