<script lang="ts">
	import { mdiCheckCircle } from '@mdi/js';

	const uid = $props.id();
</script>

<svg viewBox="0 0 24 24" aria-hidden="true">
	<defs>
		<!-- Punch the area around the front glyph out of the back one, so the
		     overlap works on any bubble background color. -->
		<mask id="cut-{uid}">
			<rect width="24" height="24" fill="white" />
			<circle cx="16" cy="12" r="7.8" fill="black" />
		</mask>
	</defs>
	<g mask="url(#cut-{uid})">
		<path
			d={mdiCheckCircle}
			fill="currentColor"
			transform="translate(0.2 4.2) scale(0.65)"
		/>
	</g>
	<g
		fill="none"
		stroke="currentColor"
		stroke-width="1.8"
		stroke-linecap="round"
	>
		<circle cx="16" cy="12" r="6" stroke-dasharray="1 3" />
		<path d="M13.4 12h4.2" />
		<path d="M15.7 9.9 17.8 12l-2.1 2.1" stroke-linejoin="round" />
	</g>
</svg>

<style>
	svg {
		width: 100%;
		height: 100%;
	}
</style>
