<script lang="ts">
	import { fly, fade } from 'svelte/transition';
	import { cubicOut } from 'svelte/easing';
	import Icon from './Icon.svelte';
	import { toast } from './toast.svelte';
	import { dur } from '$lib/motion';
	import { CheckCircle, WarningCircle, Info, X } from 'phosphor-svelte';

	const meta = {
		success: { icon: CheckCircle, tone: 'text-success', ring: 'border-success/30' },
		error: { icon: WarningCircle, tone: 'text-danger', ring: 'border-danger/30' },
		info: { icon: Info, tone: 'text-accent', ring: 'border-accent/30' }
	};
</script>

<div
	class="pointer-events-none fixed inset-x-0 bottom-0 z-[100] flex flex-col items-center gap-2 p-4 sm:items-end"
	aria-live="polite"
	aria-atomic="true"
>
	{#each toast.items as t (t.id)}
		<div
			class={`pointer-events-auto flex w-full max-w-sm items-start gap-3 rounded-xl border ${meta[t.kind].ring} bg-surface/95 px-4 py-3 shadow-lg backdrop-blur`}
			in:fly={{ y: 16, duration: dur(220), easing: cubicOut }}
			out:fade={{ duration: dur(160) }}
			role="status"
		>
			<span class={`mt-0.5 shrink-0 ${meta[t.kind].tone}`}>
				<Icon icon={meta[t.kind].icon} size={18} />
			</span>
			<p class="min-w-0 flex-1 text-sm text-fg">{t.message}</p>
			<button
				onclick={() => toast.dismiss(t.id)}
				aria-label="Dismiss"
				class="-mt-0.5 -mr-1 shrink-0 rounded-md p-1 text-muted transition-colors hover:bg-surface-2 hover:text-fg"
			>
				<Icon icon={X} size={14} />
			</button>
		</div>
	{/each}
</div>
