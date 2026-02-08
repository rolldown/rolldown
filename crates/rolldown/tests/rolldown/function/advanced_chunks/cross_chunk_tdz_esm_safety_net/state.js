// In this fixture, state starts as a const binding. When circular chunk imports exist,
// Rolldown's safety net converts const/let to var in affected chunks to avoid TDZ crashes.
export const state = {};

