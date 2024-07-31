export function test() { // `test` is not used, so `foo.js` should be removed anyway, since it is used by a function.
  require('./foo.js')
}


export const a = 1000;