exports.value = 'value';
eval("exports.then = function (resolve) { globalThis.thenCalled('eval'); resolve({}); };");
