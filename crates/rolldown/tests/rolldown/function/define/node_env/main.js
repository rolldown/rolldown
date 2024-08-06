import nodeAssert from 'node:assert'
const processNodeEnv = process.env.NODE_ENV
nodeAssert.strictEqual(processNodeEnv, 'production')