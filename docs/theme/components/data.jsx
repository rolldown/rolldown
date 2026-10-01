// Components rendering data computed at build time (see `#theme/data` in the
// doc-kit configuration).
import CodeBox from '@doc-kit/generator-react/html/ui/components/CodeBox.jsx';

import { cliHelp, nodeVersion } from '#theme/data';

/**
 * The Node.js version the repository is developed with (`.node-version`).
 */
export const NodeVersion = () => nodeVersion;

/**
 * `rolldown --help`.
 */
export const CliHelp = () => (
  <CodeBox className="language-text">
    <code>{cliHelp}</code>
  </CodeBox>
);
