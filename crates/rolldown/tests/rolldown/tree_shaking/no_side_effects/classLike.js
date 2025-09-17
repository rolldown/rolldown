/* #__NO_SIDE_EFFECTS__ */
export function classLike() {}

Object.defineProperty(classLike.prototype, 'sideEffect', {
  get() {
    console.log('this side effect may not need to be preserved? See https://github.com/javascript-compiler-hints/compiler-notations-spec/issues/8')
    return 0
  }
})
