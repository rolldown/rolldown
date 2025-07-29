// (hyf0) TODO: These types should be exported from `rolldown/hmr`

export interface HmrInvalidateMessage {
  type: 'hmr:invalidate';
  moduleId: string;
}

export type ClientMessage = HmrInvalidateMessage;
