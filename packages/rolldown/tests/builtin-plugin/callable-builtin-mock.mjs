export const nativeRoots = [];
let gate = Promise.resolve();

export function setPending(promise) {
  gate = promise;
}

function callHook(callback) {
  return gate.then(() => callback('native warning'));
}

export class BindingCallableBuiltinPlugin {
  constructor(binding) {
    const callback = binding.options.onWarn;
    nativeRoots.push(callback);
    this.resolveId = callHook.bind(undefined, callback);
  }

  getOrder() {}
}
