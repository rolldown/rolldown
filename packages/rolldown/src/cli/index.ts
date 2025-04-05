import process from 'node:process';
import { version } from '../../package.json';
import { parseCliArguments } from './arguments';
import { bundleWithCliOptions, bundleWithConfig } from './commands/bundle';
import { showHelp } from './commands/help';
import { logger } from './logger';

async function main() {
  const cliOptions = parseCliArguments();

  if (cliOptions.config || cliOptions.config === '') {
    await bundleWithConfig(cliOptions.config, cliOptions);
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
