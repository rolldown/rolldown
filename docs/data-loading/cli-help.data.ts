import { stripVTControlCharacters } from 'node:util';
import { generateHelpText } from '../../packages/rolldown/src/cli/commands/help';

export default {
  load() {
    return {
      help: stripVTControlCharacters(generateHelpText()),
    };
  },
};
