<!--
	The heading of an iOS confirmation sheet. A title on its own is not emphasised;
	a title introducing a subtitle is.
-->
<script lang="ts">
	type Props = {
		title: string;
		subtitle?: string;
	};

	let { title, subtitle }: Props = $props();
</script>

<div
	class="flex flex-col px-2.5 pb-2 pt-1 text-start text-[17px] leading-[22px] text-black dark:text-white"
>
	<span class:font-semibold={subtitle !== undefined}>{title}</span>
	{#if subtitle !== undefined}
		<span>{subtitle}</span>
	{/if}
</div>
