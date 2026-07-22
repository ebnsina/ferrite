<script lang="ts">
	import type { ComponentProps } from 'svelte';
	import { HugeiconsIcon } from '@hugeicons/svelte';

	interface Props {
		icon: ComponentProps<typeof HugeiconsIcon>['icon'];
		size?: number;
		class?: string;
	}

	let { icon, size = 18, class: klass = '' }: Props = $props();
</script>

<span class={`inline-flex ${klass}`}>
	<HugeiconsIcon {icon} {size} strokeWidth={1.8} color="currentColor" />
</span>
