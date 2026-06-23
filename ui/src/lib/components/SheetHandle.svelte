<script lang="ts">
	interface Props {
		class?: string;
	}

	let { class: className = '' }: Props = $props();
</script>

<div
	class="mt-2 h-0.5 w-10 rounded-full bg-gray-300 dark:bg-gray-600 {className}"
></div>
