import { settings, obj, counter, temp } from './constants.js'

// Property writes that should be tree-shaken when propertyWriteSideEffects: false
settings.theme = 'dark' // Should be tree-shaken
settings['language'] = 'en' // Should be tree-shaken
obj.nested.deep.value = 42 // Should be tree-shaken

o.foo = 100; // should preserve since o is a global variable

temp['bar'] = 200; // should preserve since temp is used
console.log(temp)

// Property updates that should be tree-shaken when propertyWriteSideEffects: false
counter.value++ // Should be tree-shaken
class T {
  static {
    ++counter.count // Should not be tree-shaken, since `counter` is used
  }
}

class A {
  [counter.another++] = 123 // Should not be tree-shaken, since `counter` is used
}

console.log(counter)

obj.prop-- // Should be tree-shaken
--obj.other // Should be tree-shaken

// Assignment with computed property - should NOT be tree-shaken even with propertyWriteSideEffects: false
const key = Math.random() > 0.5 ? 'a' : 'b'
obj[key] = 'computed' // Dynamic key - might have side effects

// These should remain since they're not property assignments
let localVar = 10
localVar = 20 // should be tree-shaken

// Export assignments - these should NOT be tree-shaken
export const config = {}
config.option = true // Export property write - should remain

// Object spread with property writes
const merged = { ...settings }; // TODO: Object spread should be tree-shaken when propertyReadSideEffects: false
merged.newProp = 'value' // Property write on local object - should be tree-shaken

export default {
  config
}

