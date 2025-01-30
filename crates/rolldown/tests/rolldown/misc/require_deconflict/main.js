export const __require = "test";
export default () => require("test-dep") || require(`test-dep${666}`);
