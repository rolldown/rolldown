import { bar as bar$, bar$ as bar } from './bar.js';
{

  let barb = class extends bar {
    static test() {
      assert.ok(bar.base);
    }
  };
	barb.test();

}
{
	let bar = class extends bar$ {
		static test() {
			assert.ok(bar.base);
		}
	};

	assert.strictEqual(bar.name, 'bar');
	bar.test();
}
