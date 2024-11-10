import * as ns from './foo.js'
let keys = Object.keys(ns)
input.works =
  ns.default === 123 && keys.includes('default')