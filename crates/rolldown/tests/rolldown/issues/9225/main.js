import { api } from './api.js';
import { getEnvString } from './env.js';

globalThis.__rolldown_issue_7449_imports = [
  import('./lazy.js'),
  import('./env-user.js'),
  import('./dep-user.js'),
];

globalThis.__rolldown_issue_7449_value = api + Number(getEnvString('MISSING') || 0);
