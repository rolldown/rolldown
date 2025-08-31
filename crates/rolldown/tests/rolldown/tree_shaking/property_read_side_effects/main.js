import { API_ENDPOINTS, someObject } from './constants.js'

API_ENDPOINTS.USERS // Should be tree-shaken when propertyReadSideEffects: false
API_ENDPOINTS['POSTS'] // Should be tree-shaken when propertyReadSideEffects: false

function test() {
  console.log('test function called');
}
API_ENDPOINTS[unknown] // Should not be tree-shaken when propertyReadSideEffects: false
API_ENDPOINTS[test]; // Should be tree-shaken when propertyReadSideEffects: false

(/*#__PURE__*/test()).a.b.c; // Should be tree-shaken when propertyReadSideEffects: false
test().a.b.c; // Should not be tree-shaken when propertyReadSideEffects: false

(/*#__PURE__*/test())?.a?.b.c; // Should be tree-shaken when propertyReadSideEffects: false
test()?.a?.b.c; // Should not be tree-shaken when propertyReadSideEffects: false

// Object destructuring tests
const { a, b } = someObject // Should be tree-shaken when propertyReadSideEffects: false
const { c: renamed } = someObject // Should be tree-shaken when propertyReadSideEffects: false
const { nested: { deep } } = someObject // Should be tree-shaken when propertyReadSideEffects: false

// These should remain since they have actual side effects in the initializer
const { d } = console.log('side effect') || someObject // Should NOT be tree-shaken - has side effect
const { e } = (() => { console.log('effect'); return someObject })() // Should NOT be tree-shaken

export default {}
