import { isMobile } from '$lib/utils/environment';

const mediaQuery =
	typeof window !== 'undefined'
		? window.matchMedia('(min-width: 768px) and (min-height: 500px)')
		: undefined;

let wide = $state(isMobile ? (mediaQuery?.matches ?? false) : true);

if (isMobile) {
	mediaQuery?.addEventListener('change', e => {
		wide = e.matches;
	});
}

if (typeof window !== 'undefined') {
	window.addEventListener('set-wide-screen', ((e: CustomEvent<boolean>) => {
		wide = e.detail;
	}) as EventListener);
}

export const isWideScreen = {
	get value() {
		return wide;
	},
};
