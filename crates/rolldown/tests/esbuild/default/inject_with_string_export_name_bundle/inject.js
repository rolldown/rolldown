const old = console.log
const fn = (...args) => old.apply(console, ['log:'].concat(args))
export { fn as "console.log" }
export { "console.log" as "console.info" } from "./inject.js"
import { "console.info" as info } from "./inject.js"
export { info as "console.warn" }