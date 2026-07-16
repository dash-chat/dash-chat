<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { m } from '$lib/paraglide/messages.js';
	import { mdiEmoticonHappyOutline } from '@mdi/js';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import IconButton from '$lib/components/IconButton.svelte';

	interface Props {
		onClick: () => void;
	}

	let { onClick }: Props = $props();
</script>

<IconButton {onClick} label={m.emoji()} testid="message-input-emoji">
	<wa-icon class="text-[22px]" src={wrapPathInSvg(mdiEmoticonHappyOutline)}
	></wa-icon>
</IconButton>
