// thing is a global variable, so `bar` should be unused
import("./lib.js").then(({ foo: x, bar: a }) => [x, thing]);
