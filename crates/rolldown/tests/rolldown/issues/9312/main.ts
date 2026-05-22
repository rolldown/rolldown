// The `define` plugin consumes the original Scoping; the transformer
// must still receive a Scoping built with `enum_eval` so it resolves
// the string-enum alias below to its constant value.
const env = process.env.NODE_ENV;
console.log(env);

export enum Theme {
  Light = 'Light',
  Dark = 'Dark',
  Default = Theme.Light,
}
