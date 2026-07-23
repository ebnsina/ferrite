// Typed API calls. Thin wrappers over apiRequest; components never build URLs.
import { apiRequest, API_BASE } from './client';
import type {
	ApiKey,
	Asset,
	AuthResponse,
	Job,
	LiveStream,
	Member,
	MemberInvited,
	Usage,
	User
} from './types';

// --- Auth --------------------------------------------------------------------

export function signup(email: string, password: string, workspace: string) {
	return apiRequest<AuthResponse>('/v1/auth/signup', {
		method: 'POST',
		body: JSON.stringify({ email, password, workspace })
	});
}

export function login(email: string, password: string) {
	return apiRequest<AuthResponse>('/v1/auth/login', {
		method: 'POST',
		body: JSON.stringify({ email, password })
	});
}

/** Clear the server-side session + CSRF cookies. */
export function logout() {
	return apiRequest<void>('/v1/auth/logout', { method: 'POST' });
}

export function forgotPassword(email: string) {
	return apiRequest<void>('/v1/auth/forgot-password', {
		method: 'POST',
		body: JSON.stringify({ email })
	});
}

export function resetPassword(token: string, new_password: string) {
	return apiRequest<void>('/v1/auth/reset-password', {
		method: 'POST',
		body: JSON.stringify({ token, new_password })
	});
}

// --- Team --------------------------------------------------------------------

export function listMembers() {
	return apiRequest<Member[]>('/v1/members');
}

export function inviteMember(email: string, role: 'admin' | 'member') {
	return apiRequest<MemberInvited>('/v1/members', {
		method: 'POST',
		body: JSON.stringify({ email, role })
	});
}

export function updateMemberRole(id: string, role: 'admin' | 'member') {
	return apiRequest<Member>(`/v1/members/${id}`, {
		method: 'PATCH',
		body: JSON.stringify({ role })
	});
}

export function removeMember(id: string) {
	return apiRequest<void>(`/v1/members/${id}`, { method: 'DELETE' });
}

// --- Profile -----------------------------------------------------------------

export function updateProfile(name: string) {
	return apiRequest<User>('/v1/profile', {
		method: 'PATCH',
		body: JSON.stringify({ name })
	});
}

export function changePassword(current_password: string, new_password: string) {
	return apiRequest<void>('/v1/profile/password', {
		method: 'POST',
		body: JSON.stringify({ current_password, new_password })
	});
}

export function createApiKey(name: string) {
	return apiRequest<{ id: string; prefix: string; api_key: string }>('/v1/api-keys', {
		method: 'POST',
		body: JSON.stringify({ name })
	});
}

export function listApiKeys() {
	return apiRequest<ApiKey[]>('/v1/api-keys');
}

export function revokeApiKey(id: string) {
	return apiRequest<void>(`/v1/api-keys/${id}`, { method: 'DELETE' });
}

export interface SearchHit {
	asset_id: string;
	filename: string;
	job_id: string;
	start_secs: number;
	snippet: string;
}

export function searchVideos(q: string) {
	return apiRequest<SearchHit[]>(`/v1/search?q=${encodeURIComponent(q)}`);
}

export function getUsage() {
	return apiRequest<Usage>('/v1/usage');
}

// --- Admin (superadmin only) -------------------------------------------------

export interface AdminOverview {
	tenants: number;
	users: number;
	assets: number;
	jobs: number;
	waitlist: number;
}

export function getAdminOverview() {
	return apiRequest<AdminOverview>('/v1/admin/overview');
}

export interface WaitlistRow {
	id: number;
	name: string;
	email: string;
	whatsapp: string | null;
	country: string | null;
	use_case: string | null;
	volume: string | null;
	plan: string | null;
	payment: string | null;
	created_at: string;
}

export function getAdminWaitlist() {
	return apiRequest<WaitlistRow[]>('/v1/admin/waitlist');
}

export function createLiveStream(name: string) {
	return apiRequest<LiveStream>('/v1/live/streams', {
		method: 'POST',
		body: JSON.stringify({ name })
	});
}

export function listLiveStreams() {
	return apiRequest<LiveStream[]>('/v1/live/streams');
}

export function getLiveStream(id: string) {
	return apiRequest<LiveStream>(`/v1/live/streams/${id}`);
}

export interface SimulcastTarget {
	id: string;
	name: string;
	url: string;
	stream_key: string;
	enabled: boolean;
	created_at: string;
}

export function listTargets(streamId: string) {
	return apiRequest<SimulcastTarget[]>(`/v1/live/streams/${streamId}/targets`);
}

export function createTarget(streamId: string, name: string, url: string, stream_key: string) {
	return apiRequest<SimulcastTarget>(`/v1/live/streams/${streamId}/targets`, {
		method: 'POST',
		body: JSON.stringify({ name, url, stream_key })
	});
}

export function deleteTarget(streamId: string, targetId: string) {
	return apiRequest<void>(`/v1/live/streams/${streamId}/targets/${targetId}`, { method: 'DELETE' });
}

export function clipLive(streamId: string, duration: number) {
	return apiRequest<{ asset_id: string; filename: string; status: string }>(
		`/v1/live/streams/${streamId}/clip`,
		{ method: 'POST', body: JSON.stringify({ duration }) }
	);
}

export function getMe() {
	return apiRequest<{ id: string; name: string; plan: string }>('/v1/me');
}

interface CreateAssetResponse {
	asset: Asset;
	upload_url: string;
	expires_in_secs: number;
}

export function createAsset(filename: string) {
	return apiRequest<CreateAssetResponse>('/v1/assets', {
		method: 'POST',
		body: JSON.stringify({ filename })
	});
}

export interface Provenance {
	manifest: Record<string, unknown>;
	signature: string;
	algorithm: string;
	public_key: string;
	signature_valid: boolean;
	content_matches: boolean;
	verified: boolean;
}

export function getProvenance(assetId: string) {
	return apiRequest<Provenance>(`/v1/assets/${assetId}/provenance`);
}

export interface Moderation {
	checked: boolean;
	flagged: boolean;
	categories: string[];
}

export function getModeration(assetId: string) {
	return apiRequest<Moderation>(`/v1/assets/${assetId}/moderation`);
}

export function makeShorts(id: string, count: number) {
	return apiRequest<{ job_id: string }>(`/v1/assets/${id}/shorts`, {
		method: 'POST',
		body: JSON.stringify({ count })
	});
}

export function clipAsset(id: string, start: number, end: number, name?: string) {
	return apiRequest<{ asset: Asset; job_id: string }>(`/v1/assets/${id}/clip`, {
		method: 'POST',
		body: JSON.stringify({ start, end, name: name || null })
	});
}

export function completeAsset(id: string, bytes: number) {
	return apiRequest<Asset>(`/v1/assets/${id}/complete`, {
		method: 'POST',
		body: JSON.stringify({ bytes })
	});
}

export function listAssets() {
	return apiRequest<Asset[]>('/v1/assets');
}

export function getAsset(id: string) {
	return apiRequest<Asset>(`/v1/assets/${id}`);
}

export interface TranscodeOptions {
	encrypt?: boolean;
	mp4?: boolean;
	audio?: boolean;
	captions?: boolean;
	watermark?: { position: 'tl' | 'tr' | 'bl' | 'br'; opacity: number };
}

export function createJob(assetId: string, options: TranscodeOptions = {}) {
	return apiRequest<Job>('/v1/jobs', {
		method: 'POST',
		body: JSON.stringify({ asset_id: assetId, ...options })
	});
}

export function getBrand() {
	return apiRequest<{ logo_url: string | null }>('/v1/brand');
}

export function uploadBrandLogo() {
	return apiRequest<{ upload_url: string; expires_in_secs: number }>('/v1/brand/logo', {
		method: 'POST'
	});
}

export interface BatchResult {
	submitted: Job[];
	skipped: { asset_id: string; reason: string }[];
}

export function createJobsBatch(assetIds: string[]) {
	return apiRequest<BatchResult>('/v1/jobs/batch', {
		method: 'POST',
		body: JSON.stringify({ asset_ids: assetIds })
	});
}

export function listJobs() {
	return apiRequest<Job[]>('/v1/jobs');
}

export function getJob(id: string) {
	return apiRequest<Job & { playback_url?: string }>(`/v1/jobs/${id}`);
}

export interface Cue {
	start: number;
	end: number;
	text: string;
}

export function getJobTranscript(id: string) {
	return apiRequest<Cue[]>(`/v1/jobs/${id}/transcript`);
}

export function translateCaptions(id: string, lang: string) {
	return apiRequest<{ lang: string; url: string }>(`/v1/jobs/${id}/translate`, {
		method: 'POST',
		body: JSON.stringify({ lang })
	});
}

export function getJobEmbed(id: string) {
	return apiRequest<{ embed_url: string; iframe: string }>(`/v1/jobs/${id}/embed`);
}

export interface JobAnalytics {
	views: number;
	watch_seconds: number;
	avg_view_seconds: number;
	completions: number;
	completion_rate: number;
}

export function getJobAnalytics(id: string) {
	return apiRequest<JobAnalytics>(`/v1/jobs/${id}/analytics`);
}

/**
 * Stream a job's status via SSE until it reaches a terminal state.
 * Uses fetch (not EventSource) so the session cookie rides along via
 * `credentials: 'include'`. Returns a cleanup function that aborts the stream.
 */
export function streamJob(id: string, onUpdate: (job: Job) => void): () => void {
	const controller = new AbortController();

	(async () => {
		const res = await fetch(`${API_BASE}/v1/jobs/${id}/events`, {
			credentials: 'include',
			headers: { Accept: 'text/event-stream' },
			signal: controller.signal
		});
		if (!res.ok || !res.body) return;

		const reader = res.body.getReader();
		const decoder = new TextDecoder();
		let buffer = '';
		for (;;) {
			const { value, done } = await reader.read();
			if (done) break;
			buffer += decoder.decode(value, { stream: true });
			const frames = buffer.split('\n\n');
			buffer = frames.pop() ?? '';
			for (const frame of frames) {
				const data = frame
					.split('\n')
					.find((l) => l.startsWith('data:'))
					?.slice(5)
					.trim();
				if (data) onUpdate(JSON.parse(data) as Job);
			}
		}
	})().catch((e) => {
		if (e.name !== 'AbortError') console.error('[sse]', e);
	});

	return () => controller.abort();
}

/** Upload a file's bytes directly to the presigned storage URL. */
export async function uploadToPresigned(url: string, file: File): Promise<void> {
	const res = await fetch(url, { method: 'PUT', body: file });
	if (!res.ok) throw new Error(`upload failed (${res.status})`);
}
