import fsExtra from 'fs-extra';

export async function cloneThreeJsIfNotExists() {
  if (!fsExtra.existsSync('./tmp/github/three')) {
    fsExtra.ensureDirSync('./tmp/github');
    await $`git clone --branch r108 --depth 1 https://github.com/mrdoob/three.js.git ./tmp/github/three`;
  } else {
    console.log('[skip] three.js already cloned');
  }
}

export async function cloneRolldownBenchcasesIfNotExists() {
  if (!fsExtra.existsSync('./tmp/github/rolldown-benchcases')) {
    fsExtra.ensureDirSync('./tmp/github');
    await $`git clone https://github.com/rolldown/benchcases.git ./tmp/github/rolldown-benchcases`;
  } else {
    console.log('[skip] rolldown-benchcases already cloned');
  }
}

export async function fetchRomeIfNotExists() {
  if (!fsExtra.existsSync('./tmp/github/rome')) {
    fsExtra.ensureDirSync('./tmp/github/rome');
    cd('./tmp/github/rome');
    await $`git init`;
    await $`git remote add origin https://github.com/romejs/rome.git`;
    await $`git fetch --depth 1 origin d95a3a7aab90773c9b36d9c82a08c8c4c6b68aa5`;
    await $`git checkout FETCH_HEAD`;
    cd('../../..');
  } else {
    console.log('[skip] rome already cloned');
  }
}
