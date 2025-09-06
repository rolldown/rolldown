import { execaSync } from 'execa';

import nodeOs from 'node:os';
import { rimrafSync } from 'rimraf';

export function removeDirSync(path: string) {
  if (nodeOs.platform() === 'win32') {
    // 1. Seems any nodejs-based solution to remove a directory resulted to EBUSY error on Windows
    // 2. Check if the path exists before trying to remove it, otherwise it will throw an error
    execaSync(
      `if exist "${path}" rmdir /s /q "${path}"`,
      {
        shell: true,
        stdio: 'inherit',
      },
    );
  } else {
    rimrafSync(path);
  }
}

export function sensibleTimeoutInMs(ms: number) {
  const actualMs = process.env.CI ? ms * 5 : ms;

  return new Promise<void>((resolve) => {
    setTimeout(resolve, actualMs);
  });
}
