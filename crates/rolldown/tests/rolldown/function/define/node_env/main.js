import nodeAssert from 'node:assert';
const processNodeEnv = process.env.NODE_ENV;
nodeAssert.strictEqual(processNodeEnv, 'production')

;(function (process) {
  nodeAssert.strictEqual(process.env.NODE_ENV, undefined)
})({ env: {} });
