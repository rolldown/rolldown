export let obj = {}
export let sideEffects = console.log('this should be renamed')
export let noSideEffects = /* @__PURE__ */ console.log('side effects')
export let injectedAndDefined = 'should not be used'
let injected_and_defined = 'should not be used'
export { injected_and_defined as 'injected.and.defined' }