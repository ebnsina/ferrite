<script lang="ts">
	import type { Snippet } from 'svelte';

	interface Props {
		class?: string;
		children: Snippet;
	}

	let { class: klass = '', children }: Props = $props();
</script>

<div class={`rounded-lg border border-border bg-surface p-5 ${klass}`}>
	{@render children()}
</div>
