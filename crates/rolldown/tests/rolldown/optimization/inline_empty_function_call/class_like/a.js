export let result = 0
export function classLike() {}

Object.defineProperty(classLike.prototype, 'sideEffect', {
  get() {
    result++
    return 0
  }
})
