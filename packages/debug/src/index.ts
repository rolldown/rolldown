import { type Action } from './generated';
export * from './generated';

export interface Event {
  timestamp: string;
  buildId: string;
  fields: {
    action: Action;
  };
}

export function parseToEvents(data: string): Event[] {
  return data.split('\n').map(v => JSON.parse(v));
}

export function parseToEvent(data: string): Event {
  return JSON.parse(data);
}
