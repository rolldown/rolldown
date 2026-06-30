var sharedValue = ((value) => {
  value.EventMatch = 'event_match';
  return value;
})(sharedValue || {});

export { sharedValue as SharedEnum };
