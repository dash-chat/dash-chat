<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { m } from '$lib/paraglide/messages.js';
	import { mdiCameraOutline } from '@mdi/js';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import IconButton from '$lib/components/IconButton.svelte';

	interface Props {
		onClick: () => void;
	}

	let { onClick }: Props = $props();
</script>

<IconButton {onClick} label={m.camera()} testid="message-input-camera">
	<!-- The path sits at y 2-20 of its 24-unit viewBox, so it needs a unit down
	     to be optically centred. -->
	<wa-icon
		style="font-size: 26px; transform: translateY(calc(1em / 24))"
		src={wrapPathInSvg(mdiCameraOutline)}
	></wa-icon>
</IconButton>
