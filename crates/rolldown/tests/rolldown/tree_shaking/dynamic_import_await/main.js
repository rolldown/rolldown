// the usage should be merged, rest of the exported symbol should be tree-shaken
const lib = await import("./lib.js")
lib.foo


const lib2 = await import("./lib.js")
lib.bar
