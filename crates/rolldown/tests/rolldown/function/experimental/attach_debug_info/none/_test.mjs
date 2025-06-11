import nodeAssert from "node:assert"
import nodeFs from "node:fs"
import nodePath from "node:path"

const code = nodeFs.readFileSync(nodePath.join(import.meta.dirname, "dist/main.js"), "utf-8")

nodeAssert(!code.includes('//#region'), "should not include #region")
nodeAssert(!code.includes('//#endregion'), "should not include #endregion")