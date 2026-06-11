<script lang="ts">
	import { type ComponentProps } from 'svelte';
	import { ListInput, useTheme } from 'konsta/svelte';

	type Props = Omit<ComponentProps<ListInput>, 'label' | 'value'> & {
		label?: string;
		value?: string;
	};

	let { label, value = $bindable(), ...rest }: Props = $props();

	const theme = $derived(useTheme());
</script>

<ListInput
	label={theme === 'material' ? label : ''}
	placeholder={theme === 'ios' ? label : ''}
	bind:value
	{...rest}
/>
