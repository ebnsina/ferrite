//! Video encoding parameters, switchable between CPU (libx264) and GPU (NVENC).
//!
//! Selected via `FERRITE_ENCODER`. The `Encoder` trait already abstracts the
//! pipeline from the codec; this just swaps ffmpeg's `-c:v`/preset. Scaling stays
//! on CPU for simplicity — a full GPU path (`-hwaccel cuda` + `scale_cuda`) is a
//! further optimization. Untested without NVIDIA hardware.

#[derive(Clone, Copy, Debug)]
pub struct EncodeParams {
    pub codec: &'static str,
    pub preset: &'static str,
}

impl EncodeParams {
    pub fn from_setting(encoder: &str) -> Self {
        match encoder {
            // GPU: offload H.264 encoding to NVIDIA NVENC. Requires an NVIDIA GPU
            // and drivers in the worker container.
            "nvenc" => EncodeParams {
                codec: "h264_nvenc",
                preset: "p4",
            },
            // CPU (default): software x264.
            _ => EncodeParams {
                codec: "libx264",
                preset: "veryfast",
            },
        }
    }
}
