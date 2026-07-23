<script lang="ts">
	import { fly } from 'svelte/transition';
	import { cubicOut } from 'svelte/easing';
	import { Icon } from '$lib/ui';
	import { session } from '$lib/api/session.svelte';
	import { logout } from '$lib/api/endpoints';
	import { nameFromEmail } from '$lib/format';
	import { dur } from '$lib/motion';
	import { Settings01Icon, Logout01Icon, ArrowDown01Icon } from '@hugeicons/core-free-icons';

	let open = $state(false);

	// Clear the server cookies first (best-effort), then the local identity.
	async function signOut() {
		try {
			await logout();
		} catch {
			// ignore — clear local state regardless
		}
		session.clear();
	}
	const name = $derived(session.user?.name || nameFromEmail(session.user?.email));
	const initial = $derived((session.user?.name || session.user?.email || '?').charAt(0).toUpperCase());
</script>

{#if open}
	<!-- click-away backdrop -->
	<button class="fixed inset-0 z-30 cursor-default" aria-hidden="true" onclick={() => (open = false)}
	></button>
{/if}

<div class="relative z-40">
	<button
		onclick={() => (open = !open)}
		class="flex items-center gap-2 rounded-lg p-1.5 transition-colors hover:bg-surface-2"
		aria-label="Account menu"
		aria-expanded={open}
	>
		<span
			class="flex h-8 w-8 items-center justify-center rounded-full bg-accent-soft text-sm font-semibold text-accent"
			>{initial}</span
		>
		<span class="hidden text-sm font-medium sm:block">{name}</span>
		<span class="text-muted transition-transform" class:rotate-180={open}>
			<Icon icon={ArrowDown01Icon} size={14} />
		</span>
	</button>

	{#if open}
		<div
			class="absolute right-0 mt-2 w-60 origin-top-right overflow-hidden rounded-xl border border-border bg-surface shadow-lg"
			transition:fly={{ y: -8, duration: dur(150), easing: cubicOut }}
		>
			<div class="border-b border-border p-4">
				<p class="truncate text-sm font-medium">{session.user?.email}</p>
				<p class="mt-0.5 flex items-center gap-1.5 text-xs text-muted">
					<span class="truncate">{session.tenant?.name}</span>
					<span class="rounded bg-surface-2 px-1.5 py-0.5 text-[10px] tracking-wide uppercase"
						>{session.user?.role}</span
					>
				</p>
			</div>
			<div class="p-1.5">
				<a
					href="/app/settings"
					onclick={() => (open = false)}
					class="flex items-center gap-2.5 rounded-lg px-2.5 py-2 text-sm text-muted transition-colors hover:bg-surface-2 hover:text-fg"
				>
					<Icon icon={Settings01Icon} size={16} /> Profile & settings
				</a>
				<button
					onclick={signOut}
					class="flex w-full items-center gap-2.5 rounded-lg px-2.5 py-2 text-sm text-danger transition-colors hover:bg-danger/10"
				>
					<Icon icon={Logout01Icon} size={16} /> Sign out
				</button>
			</div>
		</div>
	{/if}
</div>
