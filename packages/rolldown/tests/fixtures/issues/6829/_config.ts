import { defineTest } from "rolldown-tests";
import { expect } from "vitest";

export default defineTest({
  config: {
    input: "./main.js",
  },
  afterTest(output) {
    const chunk = output.output.find(
      (chunk) => chunk.type === "chunk" && chunk.isEntry,
    );
    expect(chunk).toBeDefined();
    if (chunk?.type === "chunk") {
      expect(chunk.code).toContain(` String.raw(_templateObject || (_templateObject = _taggedTemplateLiteral(["<\\/script>"])))`);
    }
  },
});
