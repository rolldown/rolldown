import { padZero } from './pad.js';

export function formatDate(d = new Date()) {
  return `${d.getFullYear()}-${padZero(d.getMonth() + 1)}-${padZero(d.getDate())}`;
}
