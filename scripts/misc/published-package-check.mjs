import { findWorkspacePackagesNoCheck } from "@pnpm/find-workspace-packages";
import * as path from "node:path";
import * as fs from "node:fs";
import micromatch from "micromatch";

const packagesNeedToPublish = ["packages/rolldown", "packages/rolldown/npm/*"];

const root = process.cwd();
const workspaces = await findWorkspacePackagesNoCheck(root);

workspaces.forEach((item) => {
  let absolutePath = item.dir;
  let relativePath = path.relative(root, absolutePath);
  console.log(`Checking relativePath: `, relativePath);
  if (micromatch(relativePath, packagesNeedToPublish).length > 0) {
    return;
  }

  let packageJsonPath = path.join(absolutePath, "package.json");
  let json = JSON.parse(fs.readFileSync(packageJsonPath, "utf8"));
  if (json.private) {
    return;
  }
  console.error(`Package ${relativePath} should be private`);
  process.exit(-1);
});
