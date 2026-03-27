import { send } from './logger.js';

export const doWork = () => {
  send('hello from doWork');
};
