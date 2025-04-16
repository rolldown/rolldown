//! <script>foo</script>
export let x

// Normal comments
console.log('in a') //! Copyright notice 1

console.log('in b') // Normal comments

// console.log('in c')

//! Legal comments1
/*! Legal comments2 */
foo;bar;

/**
 * test
 */
export function test() {

}

export default () => {
  /**
   * @preserve
   */
}
