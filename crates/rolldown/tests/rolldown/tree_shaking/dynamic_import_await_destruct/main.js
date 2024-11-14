// copy some tests from https://github.com/parcel-bundler/parcel/pull/5367/files
// license: https://github.com/parcel-bundler/parcel/blob/a53f8f3ba1025c7ea8653e9719e0a61ef9717079/LICENSE
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
