const ANDROID_STORE_URL =
	'https://play.google.com/store/apps/details?id=studio.darksoil.dashchat&pcampaignid=web_share';
const IOS_STORE_URL =
	'https://apps.apple.com/es/app/dash-chat-messenger/id6759798505';

export function redirectToAppStoreIfMobile() {
	const ua = navigator.userAgent;

	if (/android/i.test(ua)) {
		window.location.replace(ANDROID_STORE_URL);
	} else if (/iPad|iPhone|iPod/.test(ua) || ('maxTouchPoints' in navigator && navigator.maxTouchPoints > 1 && /Macintosh/.test(ua))) {
		window.location.replace(IOS_STORE_URL);
	}
}
