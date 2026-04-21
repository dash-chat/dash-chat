import { S } from '../selectors';

export const selectors = S.updater;

/** Return the updater banner element if present */
export function updaterBanner() {
	return document.querySelector(selectors.banner);
}

/** Return the updater banner title element if present */
export function updaterBannerTitle() {
	return document.querySelector(selectors.bannerTitle);
}

/** Return the updater dismiss button element if present */
export function updaterDismissBtn() {
	return document.querySelector(selectors.dismissBtn);
}
