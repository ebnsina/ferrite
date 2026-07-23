<script lang="ts">
	import { Button, Card, Logo } from '$lib/ui';
	import { session } from '$lib/api/session.svelte';
	import { login, signup } from '$lib/api/endpoints';
	import { ApiError } from '$lib/api/client';
	import { loginSchema, signupSchema, validate } from '$lib/schemas';

	let mode = $state<'login' | 'signup'>('login');
	let email = $state('');
	let password = $state('');
	let workspace = $state('');
	let busy = $state(false);
	let error = $state<string | null>(null);
	let fieldErrors = $state<Record<string, string>>({});

	async function submit() {
		if (busy) return;
		error = null;
		const check =
			mode === 'signup'
				? validate(signupSchema, { workspace, email, password })
				: validate(loginSchema, { email, password });
		if (!check.ok) {
			fieldErrors = check.errors;
			return;
		}
		fieldErrors = {};
		busy = true;
		try {
			const res =
				mode === 'signup'
					? await signup(email.trim(), password, workspace.trim())
					: await login(email.trim(), password);
			session.set(res);
		} catch (e) {
			error =
				e instanceof ApiError
					? e.status === 401
						? 'Invalid email or password.'
						: e.message
					: 'Something went wrong. Please try again.';
		} finally {
			busy = false;
		}
	}

	function switchMode(next: 'login' | 'signup') {
		mode = next;
		error = null;
		fieldErrors = {};
	}
</script>

<div class="flex min-h-screen items-center justify-center px-6">
	<Card class="w-full max-w-md">
		<div class="mb-6 flex justify-center">
			<Logo size={36} />
		</div>

		<h1 class="mb-1 text-center text-lg font-semibold">
			{mode === 'signup' ? 'Create your workspace' : 'Sign in'}
		</h1>
		<p class="mb-5 text-center text-sm text-muted">
			{mode === 'signup'
				? 'Start transcoding in minutes.'
				: 'Welcome back — sign in to your workspace.'}
		</p>

		<form
			onsubmit={(e) => {
				e.preventDefault();
				submit();
			}}
			class="flex flex-col gap-3"
		>
			{#if mode === 'signup'}
				<label class="flex flex-col gap-1.5">
					<span class="text-xs font-medium text-muted">Workspace name</span>
					<input
						bind:value={workspace}
						placeholder="Acme Inc"
						autocomplete="organization"
						class="w-full rounded-lg border border-border bg-surface-2 px-3 py-2 text-sm outline-none focus:border-accent"
					/>
					{#if fieldErrors.workspace}<span class="text-sm text-danger">{fieldErrors.workspace}</span>{/if}
				</label>
			{/if}

			<label class="flex flex-col gap-1.5">
				<span class="text-xs font-medium text-muted">Email</span>
				<input
					bind:value={email}
					type="email"
					placeholder="you@company.com"
					autocomplete="email"
					class="w-full rounded-lg border border-border bg-surface-2 px-3 py-2 text-sm outline-none focus:border-accent"
				/>
				{#if fieldErrors.email}<span class="text-sm text-danger">{fieldErrors.email}</span>{/if}
			</label>

			<label class="flex flex-col gap-1.5">
				<span class="text-xs font-medium text-muted">Password</span>
				<input
					bind:value={password}
					type="password"
					placeholder={mode === 'signup' ? 'At least 8 characters' : '••••••••'}
					autocomplete={mode === 'signup' ? 'new-password' : 'current-password'}
					class="w-full rounded-lg border border-border bg-surface-2 px-3 py-2 text-sm outline-none focus:border-accent"
				/>
				{#if fieldErrors.password}<span class="text-sm text-danger">{fieldErrors.password}</span>{/if}
			</label>

			{#if error}<p class="text-sm text-danger">{error}</p>{/if}

			<Button type="submit" class="mt-1 w-full" disabled={busy}>
				{busy
					? 'Please wait…'
					: mode === 'signup'
						? 'Create workspace'
						: 'Sign in'}
			</Button>
		</form>

		<div class="mt-5 text-center text-sm text-muted">
			{#if mode === 'signup'}
				Already have an account?
				<button class="text-accent hover:underline" onclick={() => switchMode('login')}>Sign in</button>
			{:else}
				New to Ferrite?
				<button class="text-accent hover:underline" onclick={() => switchMode('signup')}
					>Create a workspace</button
				>
			{/if}
		</div>
	</Card>
</div>
