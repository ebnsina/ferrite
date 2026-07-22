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
	status: 'uploading' | 'ready' | 'error';
	created_at: string;
}

export interface Job {
	id: string;
	asset_id: string;
	state: JobState;
	progress: number;
	error: string | null;
	queued_at: string;
	finished_at: string | null;
}

// Matches the API's error envelope: { error: { code, message, fields? } }
export interface ApiErrorBody {
	error: {
		code: string;
		message: string;
		fields?: unknown;
	};
}
