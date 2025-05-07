import { defineTest } from "rolldown-tests";

export default defineTest({
  config: {},
  afterTest: async () => {
    await import("./assert.mjs");
  },
});
