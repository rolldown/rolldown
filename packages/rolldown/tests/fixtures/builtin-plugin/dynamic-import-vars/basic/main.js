export function singleDir(name) {
  return import(`./dir/a/${name}.js`)
}

export function multiDirs(dir, name) {
  return import(`./dir/${dir}/${name}.js`)
}

export function noFile(name) {
  return import(`./dir/c/${name}.js`)
}

export function withAssert(name) {
  return import(`./dir/a/${name}.js`, { assert: { type: 'json' } })
}
