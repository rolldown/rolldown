import { ObjectElement } from './dist/main.js'
import assert from 'node:assert'

function hasCycleUsingJSON(obj) {
  try {
    JSON.stringify(obj);
    return false; // No cycle, stringification succeeded
  } catch (e) {
    if (e instanceof TypeError && e.message.includes('Converting circular structure to JSON')) {
      return true; // Cycle detected
    }
    throw e; // Re-throw other errors
  }
}

assert.ok(hasCycleUsingJSON(ObjectElement), 'ObjectElement should have a cycle when using JSON.stringify');
