export default function(x) {
	x.foo_;
	x.foo_?.();
	x?.foo_;
	x?.foo_();
	x?.foo_.bar_;
	x?.foo_.bar_();
	x?.['foo_'].bar_;
	x?.foo_['bar_'];
}