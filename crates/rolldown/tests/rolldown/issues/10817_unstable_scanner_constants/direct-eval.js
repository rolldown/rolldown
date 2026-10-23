let VALUE = 'rolldown';

eval(`VALUE = {
  valueOf() {
    throw new Error('direct eval valueOf called');
  }
}`);

1 * VALUE;
