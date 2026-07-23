<script lang="ts">
	import { onMount } from 'svelte';
	import { Card, Button, Icon, toast } from '$lib/ui';
	import { humanizeError } from '$lib/humanize';
	import { session } from '$lib/api/session.svelte';
	import { theme } from '$lib/theme.svelte';
	import {
		updateProfile,
		changePassword,
		getBrand,
		uploadBrandLogo,
		uploadToPresigned,
		logout
	} from '$lib/api/endpoints';
	import { ApiError } from '$lib/api/client';
	import { profileNameSchema, passwordSchema, validate } from '$lib/schemas';
	import { nameFromEmail } from '$lib/format';
	import { Sun, Moon, SignOut, Check, Image } from 'phosphor-svelte';

	const displayName = $derived(session.user?.name || nameFromEmail(session.user?.email));
	const initial = $derived((session.user?.name || session.user?.email || '?').charAt(0).toUpperCase());

	// --- Name ---
	let name = $state(session.user?.name ?? '');
	let savingName = $state(false);
	let nameErr = $state<string | null>(null);
	let nameSaved = $state(false);

	async function saveName() {
		nameErr = null;
		nameSaved = false;
		const v = validate(profileNameSchema, { name });
		if (!v.ok) return (nameErr = v.errors.name);
		savingName = true;
		try {
			const updated = await updateProfile(v.data.name);
			session.patchUser({ name: updated.name });
			nameSaved = true;
			toast.success('Your name has been updated.');
			setTimeout(() => (nameSaved = false), 2000);
		} catch (e) {
			nameErr = humanizeError(e instanceof ApiError ? e.message : null, 'Could not save.');
		} finally {
			savingName = false;
		}
	}

	// --- Password ---
	let current_password = $state('');
	let new_password = $state('');
	let confirm = $state('');
	let savingPw = $state(false);
	let pwErrors = $state<Record<string, string>>({});
	let pwSaved = $state(false);

	async function savePassword() {
		pwErrors = {};
		pwSaved = false;
		const v = validate(passwordSchema, { current_password, new_password, confirm });
		if (!v.ok) return (pwErrors = v.errors);
		savingPw = true;
		try {
			await changePassword(v.data.current_password, v.data.new_password);
			current_password = new_password = confirm = '';
			pwSaved = true;
			toast.success('Your password has been changed.');
			setTimeout(() => (pwSaved = false), 2500);
		} catch (e) {
			pwErrors = {
				current_password: humanizeError(
					e instanceof ApiError ? e.message : null,
					'Could not change password.'
				)
			};
		} finally {
			savingPw = false;
		}
	}

	// --- Brand logo (watermark source) ---
	let logoUrl = $state<string | null>(null);
	let logoBusy = $state(false);
	let logoErr = $state<string | null>(null);
	let logoInput = $state<HTMLInputElement>();

	onMount(async () => {
		try {
			logoUrl = (await getBrand()).logo_url;
		} catch {
			// non-fatal
		}
	});

	async function onLogo(e: Event) {
		const f = (e.target as HTMLInputElement).files?.[0];
		if (!f) return;
		if (!f.type.startsWith('image/')) {
			logoErr = 'Please choose an image (PNG or JPG).';
			return;
		}
		logoBusy = true;
		logoErr = null;
		try {
			const { upload_url } = await uploadBrandLogo();
			await uploadToPresigned(upload_url, f);
			logoUrl = (await getBrand()).logo_url;
			toast.success('Logo updated.');
		} catch (err) {
			logoErr = humanizeError(err instanceof ApiError ? err.message : null, 'Upload failed.');
		} finally {
			logoBusy = false;
			if (logoInput) logoInput.value = '';
		}
	}

	const modes = [
		{ value: 'light' as const, label: 'Light', icon: Sun },
		{ value: 'dark' as const, label: 'Dark', icon: Moon }
	];

	const inputCls =
		'w-full rounded-lg border border-border bg-surface-2 px-3 py-2 text-sm outline-none focus:border-accent';
</script>

<div class="mx-auto max-w-2xl">
	<div class="mb-8">
		<h1 class="text-2xl font-semibold tracking-tight">Settings</h1>
		<p class="mt-1 text-sm text-muted">Manage your profile and preferences.</p>
	</div>

	<!-- Profile -->
	<Card class="mb-6">
		<h2 class="mb-4 text-sm font-medium text-muted">Profile</h2>
		<div class="flex items-center gap-4">
			<span
				class="flex h-14 w-14 items-center justify-center rounded-full bg-accent-soft text-xl font-semibold text-accent"
				>{initial}</span
			>
			<div class="min-w-0">
				<p class="truncate text-lg font-semibold">{displayName}</p>
				<p class="truncate text-sm text-muted">{session.user?.email}</p>
			</div>
		</div>

		<div class="mt-6">
			<label for="name" class="mb-1.5 block text-xs font-medium text-muted">Display name</label>
			<div class="flex gap-2">
				<input id="name" bind:value={name} placeholder="Your name" class={inputCls} />
				<Button disabled={savingName} onclick={saveName}>
					{#if nameSaved}<Icon icon={Check} size={16} />{/if}
					{savingName ? 'Saving…' : nameSaved ? 'Saved' : 'Save'}
				</Button>
			</div>
			{#if nameErr}<p class="mt-1.5 text-sm text-danger">{nameErr}</p>{/if}
		</div>

		<dl
			class="mt-6 grid gap-px overflow-hidden rounded-lg border border-border bg-border sm:grid-cols-2"
		>
			<div class="bg-surface p-4">
				<dt class="text-xs text-muted">Workspace</dt>
				<dd class="mt-1 text-sm font-medium">{session.tenant?.name}</dd>
			</div>
			<div class="bg-surface p-4">
				<dt class="text-xs text-muted">Role</dt>
				<dd class="mt-1 text-sm font-medium capitalize">{session.user?.role}</dd>
			</div>
		</dl>
	</Card>

	<!-- Password -->
	<Card class="mb-6">
		<h2 class="mb-1 text-sm font-medium text-muted">Password</h2>
		<p class="mb-4 text-xs text-muted">Use at least 8 characters.</p>
		<div class="flex flex-col gap-3">
			<div>
				<input
					type="password"
					bind:value={current_password}
					placeholder="Current password"
					autocomplete="current-password"
					class={inputCls}
				/>
				{#if pwErrors.current_password}<p class="mt-1.5 text-sm text-danger">
						{pwErrors.current_password}
					</p>{/if}
			</div>
			<div>
				<input
					type="password"
					bind:value={new_password}
					placeholder="New password"
					autocomplete="new-password"
					class={inputCls}
				/>
				{#if pwErrors.new_password}<p class="mt-1.5 text-sm text-danger">{pwErrors.new_password}</p>{/if}
			</div>
			<div>
				<input
					type="password"
					bind:value={confirm}
					placeholder="Confirm new password"
					autocomplete="new-password"
					class={inputCls}
				/>
				{#if pwErrors.confirm}<p class="mt-1.5 text-sm text-danger">{pwErrors.confirm}</p>{/if}
			</div>
			<div class="flex items-center gap-3">
				<Button variant="secondary" disabled={savingPw} onclick={savePassword}>
					{savingPw ? 'Saving…' : 'Change password'}
				</Button>
				{#if pwSaved}<span class="flex items-center gap-1 text-sm text-success"
						><Icon icon={Check} size={15} /> Password updated</span
					>{/if}
			</div>
		</div>
	</Card>

	<!-- Brand -->
	<Card class="mb-6">
		<h2 class="mb-1 flex items-center gap-2 text-sm font-medium text-muted">
			<Icon icon={Image} size={16} /> Brand logo
		</h2>
		<p class="mb-4 text-xs text-muted">Used as the watermark on transcoded MP4 downloads.</p>
		<div class="flex items-center gap-4">
			<div
				class="flex h-16 w-28 items-center justify-center overflow-hidden rounded-lg border border-border bg-surface-2"
			>
				{#if logoUrl}
					<img src={logoUrl} alt="Brand logo" class="max-h-full max-w-full object-contain" />
				{:else}
					<span class="text-xs text-muted">No logo</span>
				{/if}
			</div>
			<div>
				<input
					bind:this={logoInput}
					type="file"
					accept="image/png,image/jpeg"
					class="hidden"
					onchange={onLogo}
				/>
				<Button variant="secondary" disabled={logoBusy} onclick={() => logoInput?.click()}>
					{logoBusy ? 'Uploading…' : logoUrl ? 'Replace logo' : 'Upload logo'}
				</Button>
				{#if logoErr}<p class="mt-2 text-sm text-danger">{logoErr}</p>{/if}
				<p class="mt-2 text-xs text-muted">PNG with transparency works best.</p>
			</div>
		</div>
	</Card>

	<!-- Appearance -->
	<Card class="mb-6">
		<h2 class="mb-1 text-sm font-medium text-muted">Appearance</h2>
		<p class="mb-4 text-xs text-muted">Choose how Ferrite looks on this device.</p>
		<div class="inline-flex rounded-lg border border-border p-1">
			{#each modes as m (m.value)}
				<button
					onclick={() => theme.set(m.value)}
					class={`flex items-center gap-2 rounded-md px-4 py-2 text-sm font-medium transition-colors ${
						theme.mode === m.value ? 'bg-accent-soft text-accent' : 'text-muted hover:text-fg'
					}`}
				>
					<Icon icon={m.icon} size={16} />
					{m.label}
				</button>
			{/each}
		</div>
	</Card>

	<!-- Session -->
	<Card>
		<h2 class="mb-1 text-sm font-medium text-muted">Session</h2>
		<p class="mb-4 text-xs text-muted">Sign out of Ferrite on this device.</p>
		<Button variant="secondary" onclick={async () => { await logout().catch(() => {}); session.clear(); }}>
			<Icon icon={SignOut} size={16} /> Sign out
		</Button>
	</Card>
</div>
