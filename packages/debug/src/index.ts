import { type Meta } from './generated/index.js';
export * from './generated/index.js';

export interface Event {
  timestamp: string;
  session_id: string;
  meta: Meta;
}

export function parseToEvents(data: string): Event[] {
  return data.split('\n').map(v => JSON.parse(v));
}

export function parseToEvent(data: string): Event {
  return JSON.parse(data);
}
