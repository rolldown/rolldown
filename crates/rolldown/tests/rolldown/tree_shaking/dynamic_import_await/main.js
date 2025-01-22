// copy some tests from https://github.com/parcel-bundler/parcel/pull/5367/files
// license: https://github.com/parcel-bundler/parcel/blob/a53f8f3ba1025c7ea8653e9719e0a61ef9717079/LICENSE


// the usage should be merged, rest of the exported symbol should be tree-shaken
const lib = await import("./lib.js")
lib.foo


const lib2 = await import("./lib.js")
lib.bar


;(await import("./lib.js"))['baz']

// this should not bailout
;(await import("./lib.js")); 
