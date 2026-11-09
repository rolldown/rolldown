import { stripVTControlCharacters } from 'node:util';
import { generateHelpText } from '../../packages/rolldown/src/cli/commands/help.ts';

export default {
  load() {
    return {
      help: stripVTControlCharacters(generateHelpText()),
    };
  },
};
