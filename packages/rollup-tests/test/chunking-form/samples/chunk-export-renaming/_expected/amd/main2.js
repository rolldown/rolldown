define(['exports', './generated-one'], function (exports, one) {
  'use strict'

  class Two {
    test() {
      return one.ONE_CONSTANT
    }
  }

  exports.ItemTwo = Two
})
