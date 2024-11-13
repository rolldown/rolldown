// @cSpell: disable
// the usage should be merged, rest of the exported symbol should be tree-shaken
const {foo: x, thing: a} = await import("./lib.js")
console.log(x);


async function test() {
  // FIXME: this is sub optimal, should not reference `barbarbar`
  // since the `await` is unreachable
  const {thing: a, bar: barbarbar} = await import("./lib.js")
  barbarbar
}
