<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import { mdiMicrophone } from '@mdi/js';
	import IconButton from '$lib/components/IconButton.svelte';

	interface Props {
		onClick?: () => void;
	}

	let { onClick = () => {} }: Props = $props();
</script>

<IconButton
	{onClick}
	icon={mdiMicrophone}
	label={m.recordVoiceMessage()}
	testid="message-input-record-voice"
/>
