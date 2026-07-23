// Shared API contract types.
//
// TODO(phase-1): generate these from the Rust structs (via `ts-rs` or an
// OpenAPI spec) so the frontend and backend can never drift. Hand-written for now.

export type JobState =
	| 'queued'
	| 'probing'
	| 'transcoding'
	| 'packaging'
	| 'uploading'
	| 'completed'
	| 'failed';

export interface Asset {
	id: string;
	filename: string;
	bytes: number | null;
	status: 'uploading' | 'processing' | 'ready' | 'error';
	created_at: string;
	/** Signed, embeddable derived-media URLs (present once ready). */
	thumbnail_url: string | null;
	preview_url: string | null;
}

export interface Job {
	id: string;
	asset_id: string;
	state: JobState;
	progress: number;
	error: string | null;
	queued_at: string;
	finished_at: string | null;
	playback_url?: string;
	dash_url?: string;
	poster_url?: string;
	storyboard_url?: string;
}

export interface Usage {
	minutes: number;
	storage_bytes: number;
	storage_gb: number;
	cost: { currency: string; transcode: number; storage: number; total: number };
}

export interface LiveStream {
	id: string;
	name: string;
	stream_key: string;
	ingest_url: string;
	srt_url: string;
	hls_url: string;
	flv_url: string;
	created_at: string;
	live: boolean;
}

export interface User {
	id: string;
	email: string;
	name: string | null;
	role: string;
}

export interface AuthResponse {
	token: string;
	user: User;
	tenant: { id: string; name: string };
}

export interface Member {
	id: string;
	email: string;
	name: string | null;
	role: string;
	created_at: string;
}

export interface MemberInvited {
	member: Member;
	temp_password: string;
}

export interface ApiKey {
	id: string;
	name: string;
	prefix: string;
	last_used_at: string | null;
	revoked: boolean;
	created_at: string;
}

// Matches the API's error envelope: { error: { code, message, fields? } }
export interface ApiErrorBody {
	error: {
		code: string;
		message: string;
		fields?: unknown;
	};
}
