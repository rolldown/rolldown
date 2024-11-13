import * as ns from './foo.js'
let keys = Object.keys(ns)
input.works = ns.foo === 123 &&
  keys.includes('foo') && !keys.includes('default')