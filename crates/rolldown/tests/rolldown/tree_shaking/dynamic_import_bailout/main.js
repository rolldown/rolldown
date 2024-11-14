// copy some tests from https://github.com/parcel-bundler/parcel/pull/5367/files
// license: https://github.com/parcel-bundler/parcel/blob/a53f8f3ba1025c7ea8653e9719e0a61ef9717079/LICENSE
import("./lib.js").then((ns) => [ns.foo, ns.thing, ns]);
