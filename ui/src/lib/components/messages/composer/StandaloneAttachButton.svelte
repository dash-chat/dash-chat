<script lang="ts">
	import { useTheme } from 'konsta/svelte';
	import { isMobile } from '$lib/utils/environment';
	import AttachButton from '$lib/components/messages/composer/AttachButton.svelte';

	interface Props {
		onClick?: () => void;
		/** Reflects the open state of the menu/panel the caller opens. */
		expanded?: boolean;
	}

	let { onClick = () => {}, expanded = false }: Props = $props();

	const theme = $derived(useTheme());
	const glass = $derived(theme === 'ios');
	const brand = $derived(!glass && isMobile);

	const surfaceClass = $derived(
		glass
			? '!h-[42px] !w-[42px] !bg-ios-light-glass !opacity-100 shadow-ios-light-glass backdrop-blur-lg dark:!bg-ios-dark-glass dark:shadow-ios-dark-glass'
			: brand
				? '!h-[42px] !w-[42px] !bg-brand-primary !opacity-100'
				: '',
	);
</script>

<div class="contents {brand ? 'text-white' : ''}">
	<AttachButton {onClick} {expanded} class={surfaceClass} />
</div>
