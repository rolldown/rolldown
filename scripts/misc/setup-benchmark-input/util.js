import fsExtra from 'fs-extra'

let gitPathCache = ''

/**
 * @param {string} command
 * @returns {Promise<boolean>}
 */
async function validateGitExecutable(command) {
  try {
    // if git executable not found, panic.
    await $`${command} --version`
    return true
  } catch {
    return false
  }
}

async function findGitExecutable() {
  if (gitPathCache) return gitPathCache

  const candidates = process.platform === 'win32' ? ['git.exe', 'git'] : ['git']
  for (const candidate of candidates) {
    if (await validateGitExecutable(candidate)) {
      gitPathCache = candidate
      return gitPathCache
    }
  }

  throw new Error('Git executable not found.')
}

export async function cloneThreeJsIfNotExists() {
  if (!fsExtra.existsSync('./tmp/github/three')) {
    fsExtra.ensureDirSync('./tmp/github')
    await $`${await findGitExecutable()} clone --branch r108 --depth 1 https://github.com/mrdoob/three.js.git ./tmp/github/three`
  } else {
    console.log('[skip] three.js already cloned')
  }
}

export async function cloneRolldownBenchcasesIfNotExists() {
  if (!fsExtra.existsSync('./tmp/github/rolldown-benchcases')) {
    fsExtra.ensureDirSync('./tmp/github')
    await $`${await findGitExecutable()} clone https://github.com/rolldown/benchcases.git ./tmp/github/rolldown-benchcases`
  } else {
    console.log('[skip] rolldown-benchcases already cloned')
  }
}

export async function fetchRomeIfNotExists() {
  const gitPath = await findGitExecutable()
  if (!fsExtra.existsSync('./tmp/github/rome')) {
    fsExtra.ensureDirSync('./tmp/github/rome')
    cd('./tmp/github/rome')
    await $`${gitPath} init`
    await $`${gitPath} remote add origin https://github.com/romejs/rome.git`
    await $`${gitPath} fetch --depth 1 origin d95a3a7aab90773c9b36d9c82a08c8c4c6b68aa5`
    await $`${gitPath} checkout FETCH_HEAD`
    cd('../../..')
  } else {
    console.log('[skip] rome already cloned')
  }
}
