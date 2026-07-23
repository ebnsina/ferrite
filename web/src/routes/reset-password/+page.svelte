<script lang="ts">
	import { page } from '$app/state';
	import { Button, Card, Logo, Icon, toast } from '$lib/ui';
	import { resetPassword } from '$lib/api/endpoints';
	import { ApiError } from '$lib/api/client';
	import { humanizeError } from '$lib/humanize';
	import { resetSchema, validate } from '$lib/schemas';
	import { Tick02Icon } from '@hugeicons/core-free-icons';

	const token = $derived(page.url.searchParams.get('token') ?? '');

	let new_password = $state('');
	let confirm = $state('');
	let busy = $state(false);
	let errors = $state<Record<string, string>>({});
	let done = $state(false);

	async function submit() {
		errors = {};
		if (!token) return (errors = { new_password: 'This reset link is invalid or incomplete.' });
		const v = validate(resetSchema, { new_password, confirm });
		if (!v.ok) return (errors = v.errors);
		busy = true;
		try {
			await resetPassword(token, v.data.new_password);
			done = true;
			toast.success('Your password has been reset. You can sign in now.');
		} catch (e) {
			errors = {
				new_password: humanizeError(
					e instanceof ApiError ? e.message : null,
					'Could not reset your password. Try again.'
				)
			};
		} finally {
			busy = false;
		}
	}

	const inputCls =
		'w-full rounded-lg border border-border bg-surface-2 px-3 py-2 text-sm outline-none focus:border-accent';
</script>

<svelte:head><title>Reset password — Ferrite</title></svelte:head>

<div class="flex min-h-screen items-center justify-center px-6">
	<Card class="w-full max-w-md">
		<div class="mb-6 flex justify-center"><Logo size={36} /></div>

		{#if done}
			<div class="text-center">
				<span
					class="mx-auto mb-4 flex h-12 w-12 items-center justify-center rounded-full bg-success/10 text-success"
				>
					<Icon icon={Tick02Icon} size={24} />
				</span>
				<h1 class="text-lg font-semibold">Password updated</h1>
				<p class="mt-1 mb-6 text-sm text-muted">You can now sign in with your new password.</p>
				<a href="/app"><Button class="w-full">Go to sign in</Button></a>
			</div>
		{:else}
			<h1 class="mb-1 text-center text-lg font-semibold">Choose a new password</h1>
			<p class="mb-5 text-center text-sm text-muted">Enter a new password for your account.</p>
			<form
				onsubmit={(e) => {
					e.preventDefault();
					submit();
				}}
				class="flex flex-col gap-3"
			>
				<div>
					<input
						type="password"
						bind:value={new_password}
						placeholder="New password"
						autocomplete="new-password"
						class={inputCls}
					/>
					{#if errors.new_password}<p class="mt-1.5 text-sm text-danger">{errors.new_password}</p>{/if}
				</div>
				<div>
					<input
						type="password"
						bind:value={confirm}
						placeholder="Confirm new password"
						autocomplete="new-password"
						class={inputCls}
					/>
					{#if errors.confirm}<p class="mt-1.5 text-sm text-danger">{errors.confirm}</p>{/if}
				</div>
				<Button type="submit" class="mt-1 w-full" disabled={busy}>
					{busy ? 'Saving…' : 'Reset password'}
				</Button>
			</form>
			<div class="mt-5 text-center text-sm text-muted">
				<a href="/app" class="text-accent hover:underline">Back to sign in</a>
			</div>
		{/if}
	</Card>
</div>
