<script lang="ts">
	import type { Component } from 'svelte';

	type IconWeight = 'thin' | 'light' | 'regular' | 'bold' | 'fill' | 'duotone';

	interface Props {
		/** A phosphor-svelte icon component. */
		icon: Component<any>;
		size?: number;
		weight?: IconWeight;
		class?: string;
	}

	// Rename to a capitalized binding so it can be rendered as a component.
	let { icon: IconCmp, size = 18, weight = 'regular', class: klass = '' }: Props = $props();
</script>

<span class={`inline-flex ${klass}`}>
	<IconCmp {size} {weight} color="currentColor" />
</span>
