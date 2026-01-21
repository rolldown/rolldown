import nodeFs from "node:fs";
import nodeAssert from "node:assert";
import nodePath from "node:path";

const content = nodeFs.readFileSync(nodePath.join(import.meta.dirname, "dist/main.js"), "utf-8");

// Template literals containing </script> should be emitted with it escaped as <\/script> to avoid breaking HTML script tags
nodeAssert(content.includes("String.raw`<\\/script>`"));
nodeAssert(content.includes("`<\\/script>`"));
