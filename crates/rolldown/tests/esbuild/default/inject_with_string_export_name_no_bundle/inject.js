const old = console.log
const fn = (...args) => old.apply(console, ['log:'].concat(args))
export { fn as "console.log" }