import { api } from './api.js';
import { getEnvString } from './env.js';

void import('./lazy.js');
void import('./env-user.js');
void import('./dep-user.js');

globalThis.__rolldown_issue_7449_value = api + Number(getEnvString('MISSING') || 0);
