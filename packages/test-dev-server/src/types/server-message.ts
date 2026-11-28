export interface HmrUpdateMessage {
  type: 'hmr:update';
  changedIds: string[];
  staleIds: string[];
  gen: number;
}

export interface ConnectedMessage {
  type: 'connected';
}
