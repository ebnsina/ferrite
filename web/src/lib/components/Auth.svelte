<script lang="ts">
	import { goto } from '$app/navigation';
	import { Button, Card, Logo } from '$lib/ui';
	import { session } from '$lib/api/session.svelte';
	import { login, signup, forgotPassword } from '$lib/api/endpoints';
	import { ApiError } from '$lib/api/client';
	import { humanizeError } from '$lib/humanize';
	import { loginSchema, signupSchema, forgotSchema, validate } from '$lib/schemas';

	type Mode = 'login' | 'signup' | 'forgot';
	let mode = $state<Mode>('login');
	let email = $state('');
	let password = $state('');
	let workspace = $state('');
	let busy = $state(false);
	let error = $state<string | null>(null);
	let fieldErrors = $state<Record<string, string>>({});
	let forgotSent = $state(false);

	async function submit() {
		if (busy) return;
		error = null;

		if (mode === 'forgot') {
			const check = validate(forgotSchema, { email });
			if (!check.ok) return (fieldErrors = check.errors);
			fieldErrors = {};
			busy = true;
			try {
				await forgotPassword(email.trim());
				forgotSent = true;
			} catch (e) {
				error = humanizeError(e instanceof ApiError ? e.message : null);
			} finally {
				busy = false;
			}
			return;
		}

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
			// The auth token is set as an HttpOnly cookie by the response; we keep
			// only non-sensitive identity for the UI.
			session.set({ user: res.user, tenant: res.tenant });
			// Role-based redirect: superadmins land in the admin console.
			if (res.user.superadmin) goto('/admin');
		} catch (e) {
			error =
				e instanceof ApiError && e.status === 401
					? 'Invalid email or password.'
					: humanizeError(e instanceof ApiError ? e.message : null);
		} finally {
			busy = false;
		}
	}

	function switchMode(next: Mode) {
		mode = next;
		error = null;
		fieldErrors = {};
		forgotSent = false;
	}
</script>

<div class="flex min-h-screen items-center justify-center px-6">
	<Card class="w-full max-w-md">
		<div class="mb-6 flex justify-center">
			<Logo size={36} />
		</div>

		<h1 class="mb-1 text-center text-lg font-semibold">
			{mode === 'signup' ? 'Create your workspace' : mode === 'forgot' ? 'Reset your password' : 'Sign in'}
		</h1>
		<p class="mb-5 text-center text-sm text-muted">
			{mode === 'signup'
				? 'Start transcoding in minutes.'
				: mode === 'forgot'
					? "Enter your email and we'll send a reset link."
					: 'Welcome back — sign in to your workspace.'}
		</p>

		{#if mode === 'forgot' && forgotSent}
			<div class="rounded-lg border border-success/30 bg-success/10 p-4 text-center text-sm">
				If an account exists for <span class="font-medium">{email}</span>, a reset link is on its
				way. Check your inbox.
			</div>
			<div class="mt-5 text-center text-sm text-muted">
				<button class="text-accent hover:underline" onclick={() => switchMode('login')}
					>Back to sign in</button
				>
			</div>
		{:else}
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

				{#if mode !== 'forgot'}
					<label class="flex flex-col gap-1.5">
						<div class="flex items-center justify-between">
							<span class="text-xs font-medium text-muted">Password</span>
							{#if mode === 'login'}
								<button
									type="button"
									class="text-xs text-accent hover:underline"
									onclick={() => switchMode('forgot')}>Forgot?</button
								>
							{/if}
						</div>
						<input
							bind:value={password}
							type="password"
							placeholder={mode === 'signup' ? 'At least 8 characters' : '••••••••'}
							autocomplete={mode === 'signup' ? 'new-password' : 'current-password'}
							class="w-full rounded-lg border border-border bg-surface-2 px-3 py-2 text-sm outline-none focus:border-accent"
						/>
						{#if fieldErrors.password}<span class="text-sm text-danger">{fieldErrors.password}</span>{/if}
					</label>
				{/if}

				{#if error}<p class="text-sm text-danger">{error}</p>{/if}

				<Button type="submit" class="mt-1 w-full" disabled={busy}>
					{busy
						? 'Please wait…'
						: mode === 'signup'
							? 'Create workspace'
							: mode === 'forgot'
								? 'Send reset link'
								: 'Sign in'}
				</Button>
			</form>

			<div class="mt-5 text-center text-sm text-muted">
				{#if mode === 'signup'}
					Already have an account?
					<button class="text-accent hover:underline" onclick={() => switchMode('login')}>Sign in</button>
				{:else if mode === 'forgot'}
					Remembered it?
					<button class="text-accent hover:underline" onclick={() => switchMode('login')}
						>Back to sign in</button
					>
				{:else}
					New to Ferrite Stream?
					<button class="text-accent hover:underline" onclick={() => switchMode('signup')}
						>Create a workspace</button
					>
				{/if}
			</div>
		{/if}
	</Card>
</div>
