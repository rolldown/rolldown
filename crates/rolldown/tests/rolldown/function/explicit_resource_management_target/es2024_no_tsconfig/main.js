function acquireResource() {
  return { [Symbol.dispose]() {} };
}

using resource = acquireResource();
console.log('acquired', resource);
