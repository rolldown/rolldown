export const variable = (name) =>
  import(`./files/${name}.js`, { with: { tag: 'variable', __proto__: 'string' } });
export const variableComputed = (name) =>
  import(`./files/${name}.js`, { with: { tag: 'variable-computed', ['__proto__']: 'string' } });
