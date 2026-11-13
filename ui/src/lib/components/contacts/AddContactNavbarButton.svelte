<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { mdiQrcode } from '@mdi/js';
	import { Link } from 'konsta/svelte';
	import { goto } from '$app/navigation';
	import { m } from '$lib/paraglide/messages';
	import { isWideScreen } from '$lib/stores/screen.svelte';
	import { wrapPathInSvg } from '$lib/utils/icon';
</script>

<Link
	iconOnly
	aria-label={m.addContact()}
	data-testid="home-add-contact-btn"
	onClick={() =>
		isWideScreen.value
			? goto('/settings/profile/add-contact')
			: // The sidebar's back button returns to where the user came from
				// only for an entry carrying this state; otherwise it goes home.
				goto('/new-message/add-contact', {
					state: { sidebarPanel: 'new-message' },
				})}
>
	<wa-icon src={wrapPathInSvg(mdiQrcode)}> </wa-icon>
</Link>
