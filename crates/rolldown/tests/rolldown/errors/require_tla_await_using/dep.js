// `await using` at module scope awaits the disposer when the module body completes, so this
// module has top-level await even though it contains no AwaitExpression.
const resource = { async [Symbol.asyncDispose]() {} };
await using handle = resource;
export { handle };
