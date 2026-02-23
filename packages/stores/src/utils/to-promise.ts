import { ReactiveFn, ReactivePromise, watcher } from "signalium";


export function toPromise<T, Args extends unknown[]>(
	promise: ReactiveFn<ReactivePromise<T>, Args>,
	...args: Args
): Promise<T> {
	return new Promise((resolve, reject) => {
		let l: (() => void) | undefined;

		const w = watcher(
			() => {
				const value = promise(...args);
				if (value.isResolved) resolve(value.value!);
				if (value.isRejected) reject(value.error);
				if (!value.isPending && l) l();
			},
			{
				// TODO: write a more optimized version of the equality evaluator?
				equals: () => false,
			},
		);
		l = w.addListener(() => {});
	});
}
