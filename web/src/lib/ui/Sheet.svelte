<script lang="ts">
	import type { Snippet } from 'svelte';
	import { fade, fly } from 'svelte/transition';
	import { cubicOut } from 'svelte/easing';
	import { Icon } from '$lib/ui';
	import { dur } from '$lib/motion';
	import { X } from 'phosphor-svelte';

	interface Props {
		open: boolean;
		title: string;
		description?: string;
		onclose: () => void;
		children: Snippet;
		footer?: Snippet;
	}
	let { open, title, description, onclose, children, footer }: Props = $props();

	function onkeydown(e: KeyboardEvent) {
		if (e.key === 'Escape') onclose();
	}

	// Lock background scroll while open, compensating for the scrollbar width so
	// the page behind doesn't shift (the "jump").
	$effect(() => {
		if (!open) return;
		const gutter = window.innerWidth - document.documentElement.clientWidth;
		const prevOverflow = document.body.style.overflow;
		const prevPad = document.body.style.paddingRight;
		document.body.style.overflow = 'hidden';
		if (gutter > 0) document.body.style.paddingRight = `${gutter}px`;
		return () => {
			document.body.style.overflow = prevOverflow;
			document.body.style.paddingRight = prevPad;
		};
	});
</script>

<svelte:window {onkeydown} />

{#if open}
	<!-- Backdrop -->
	<button
		class="fixed inset-0 z-40 bg-black/50 backdrop-blur-sm"
		aria-label="Close"
		onclick={onclose}
		transition:fade={{ duration: dur(180) }}
	></button>

	<!-- Panel -->
	<div
		class="fixed inset-y-0 right-0 z-50 flex w-full max-w-md flex-col border-l border-border bg-bg shadow-2xl"
		role="dialog"
		aria-modal="true"
		aria-label={title}
		transition:fly={{ x: 520, duration: dur(280), easing: cubicOut }}
	>
		<div class="flex items-start justify-between border-b border-border px-6 py-5">
			<div>
				<h2 class="text-lg font-semibold tracking-tight">{title}</h2>
				{#if description}<p class="mt-1 text-sm text-muted">{description}</p>{/if}
			</div>
			<button
				onclick={onclose}
				aria-label="Close"
				class="rounded-lg p-1.5 text-muted transition-colors hover:bg-surface-2 hover:text-fg"
			>
				<Icon icon={X} size={18} />
			</button>
		</div>

		<div class="flex-1 overflow-y-auto px-6 py-6">
			{@render children()}
		</div>

		{#if footer}
			<div class="border-t border-border px-6 py-4">
				{@render footer()}
			</div>
		{/if}
	</div>
{/if}
