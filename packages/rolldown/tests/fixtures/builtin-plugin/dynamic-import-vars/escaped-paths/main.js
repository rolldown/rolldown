export function dynamic(name) {
  return import(`./dir/a"b.\`\${literal}.${name}.js`);
}

export function dynamicWithCookedPath(name) {
  return import(`./\u0064ir/a\"b.\`\${literal}.${name}.\u006as`);
}

export function dynamicWithAlias(name) {
  return import(`@quoted/${name}.js`);
}

export function dynamicWithQuery(name) {
  return import(`./dir/a"b.\`\${literal}.${name}.js?value="\n`);
}

export function dynamicWithEscapedRawQuery(name) {
  return import(`./dir/${name}.js?raw&value="quoted"\\path\n`);
}
