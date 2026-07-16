<script lang="ts">
	import { useTheme } from 'konsta/svelte';
	import { isMobile } from '$lib/utils/environment';
	import AttachButton from '$lib/components/messages/composer/AttachButton.svelte';

	interface Props {
		onClick?: () => void;
		/** Bindable; forwarded to AttachButton. Omit to let the button own it. */
		expanded?: boolean;
	}

	let { onClick = () => {}, expanded = $bindable(false) }: Props = $props();

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

<AttachButton
	{onClick}
	bind:expanded
	class={surfaceClass}
	iconClass={brand ? 'text-white' : ''}
/>
