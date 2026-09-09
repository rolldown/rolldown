const resource = { async [Symbol.asyncDispose]() {} };
await using handle = resource;
export { handle };
