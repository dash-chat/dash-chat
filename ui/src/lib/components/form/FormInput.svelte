<script lang="ts">
	import { ListInput, useTheme } from 'konsta/svelte';

	type OwnProps = { label?: string; value?: string };
	type PassthroughProps = Omit<
		Record<string, unknown>,
		'label' | 'placeholder'
	>;

	let {
		label,
		value = $bindable(),
		...rest
	}: OwnProps & PassthroughProps = $props();

	const theme = $derived(useTheme());
</script>

<ListInput
	label={theme === 'material' ? label : ''}
	placeholder={theme === 'ios' ? label : ''}
	{...rest}
/>
