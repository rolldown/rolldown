import { spawn } from "child_process";
import path from 'path'

import {isWindows} from './src/util.js'

if (isWindows()) {
  // TODO: enable this test on Windows
  process.exit(0);
}

const __dirname = path.dirname(new URL(import.meta.url).pathname);

function diffDirectories(dir1, dir2) {
  return new Promise((resolve, reject) => {
    const diffProcess = spawn("diff", ["-qr", dir1, dir2], { stdio: "inherit" });
    diffProcess.on("exit", (code) => {
      if (code === 0) {
        console.log("✅ Directories are identical.");
        resolve();
      } else if (code === 1) {
        console.log("❌ Directories differ.");
        reject();
      } else {
        console.log("! An error occurred.");
        reject();
      }
    });
  });
}

let hasError = false;

for (let i = 0; i < 9; i++) {
  try {
    await diffDirectories(path.resolve(__dirname, `dist${i}`), path.resolve(__dirname, `dist${i + 1}`));
  } catch {
    hasError = true;
  }
}

if (hasError) {
  process.exit(1);
}
