<script lang="ts">
	// Ferrite Stream brand mark: a bold "F" monogram in an accent tile + wordmark.
	// Custom SVG (not an icon-lib glyph) so the brand is ownable.
	interface Props {
		size?: number;
		withWord?: boolean;
		class?: string;
	}
	let { size = 28, withWord = true, class: klass = '' }: Props = $props();
</script>

<span class={`inline-flex items-center gap-2.5 ${klass}`}>
	<svg
		width={size}
		height={size}
		viewBox="0 0 32 32"
		fill="none"
		class="shrink-0 text-accent"
		aria-hidden="true"
	>
		<rect width="32" height="32" rx="8" fill="currentColor" />
		<path
			d="M11 8 H22 V12 H15 V15 H20.5 V19 H15 V24 H11 Z"
			fill="#fff"
			stroke="#fff"
			stroke-width="0.5"
			stroke-linejoin="round"
		/>
	</svg>
	{#if withWord}
		<span class="text-lg font-semibold tracking-tight">Ferrite Stream</span>
	{/if}
</span>
