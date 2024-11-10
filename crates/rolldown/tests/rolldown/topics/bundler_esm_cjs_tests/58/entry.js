import * as ns from './foo.js'
input.works = ns.foo === 123 &&
  {}.hasOwnProperty.call(ns, 'foo') &&
  !{}.hasOwnProperty.call(ns, 'default')