import { goto } from '$app/navigation';

import {
	HTTPS_DEEP_LINK_BASE_URL,
	SCHEME_DEEP_LINK_BASE_URL,
} from './constants';

export const path = '/add-contact/{{code}}';

const pathPrefix = path.replace('{{code}}', '');
const httpsDeepLinkPrefix = HTTPS_DEEP_LINK_BASE_URL + pathPrefix;
const schemeDeepLinkPrefix = SCHEME_DEEP_LINK_BASE_URL + pathPrefix.slice(1);

export function toDeepLink(code: string): string {
	return httpsDeepLinkPrefix + code;
}

export function extractCodeFromDeepLink(input: string): string {
	if (input.startsWith(httpsDeepLinkPrefix))
		return input.slice(httpsDeepLinkPrefix.length);
	if (input.startsWith(schemeDeepLinkPrefix))
		return input.slice(schemeDeepLinkPrefix.length);
	return input;
}

export function handle({ code }: Record<string, string>) {
	goto(`/new-message/add-contact?code=${encodeURIComponent(code)}`);
}
