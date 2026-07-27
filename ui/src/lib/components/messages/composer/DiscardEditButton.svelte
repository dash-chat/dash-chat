<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import { mdiClose } from '@mdi/js';
	import { useTheme } from 'konsta/svelte';
	import { isWideScreen } from '$lib/stores/screen.svelte';
	import IconButton from '$lib/components/IconButton.svelte';

	interface Props {
		onClick: () => void;
	}

	let { onClick }: Props = $props();

	const theme = $derived(useTheme());
</script>

<IconButton
	icon={mdiClose}
	{onClick}
	label={m.cancel()}
	testid="composer-cancel-edit"
	class={isWideScreen.value
		? '!bg-[#848484] !text-white !opacity-100'
		: `!h-[42px] !w-[42px] !opacity-100 ${
				theme === 'ios'
					? '!bg-ios-light-glass shadow-ios-light-glass backdrop-blur-lg dark:!bg-ios-dark-glass dark:shadow-ios-dark-glass'
					: '!bg-[#545863] !text-white dark:!bg-[#BEBFC5] dark:!text-[#1E2438]'
			}`}
/>
