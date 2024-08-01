export default 'entry chunk'

export function f() {
  import('./chunk.js')
}
