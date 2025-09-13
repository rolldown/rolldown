import process from 'node:process';
import { version } from '../../package.json';
import { parseCliArguments } from './arguments';
import { bundleWithCliOptions, bundleWithConfig } from './commands/bundle';
import { showHelp } from './commands/help';
import { logger } from './logger';
import { checkNodeVersion } from './version-check';

if (!checkNodeVersion(process.versions.node)) {
  logger.warn(
    `You are using Node.js ${process.versions.node}. ` +
      `Rolldown requires Node.js version 20.19+ or 22.12+. ` +
      `Please upgrade your Node.js version.`,
  );
}

async function main() {
  const { rawArgs, ...cliOptions } = parseCliArguments();

  if (cliOptions.config || cliOptions.config === '') {
    await bundleWithConfig(cliOptions.config, cliOptions, rawArgs);
    return;
  }

  if ('input' in cliOptions.input) {
    // If input is specified, we will bundle with the input options
    await bundleWithCliOptions(cliOptions);
    return;
  }

  if (cliOptions.version) {
    logger.log(`rolldown v${version}`);
    return;
  }

  showHelp();
}

main().catch((err: unknown) => {
  logger.error(err);
  process.exit(1);
});
