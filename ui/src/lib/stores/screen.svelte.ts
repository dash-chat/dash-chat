const mediaQuery =
	typeof window !== 'undefined'
		? window.matchMedia('(min-width: 768px) and (min-height: 500px)')
		: undefined;

let wide = $state(mediaQuery?.matches ?? false);

mediaQuery?.addEventListener('change', (e) => {
	wide = e.matches;
});

export const isWideScreen = {
	get value() {
		return wide;
	},
};
