import * as ns from './share.js'
import('./dynamic.js').then(mod => {
  mod.c
})
console.log(ns.a)

