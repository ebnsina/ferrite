//! The unit of work a machine picks up.
//!
//! Serialisable end to end: the scheduler treats `spec` as opaque bytes, so
//! whatever describes a job has to survive the round trip to a worker that
//! shares no memory with whoever submitted it.

use ferrite_av::split::{Chunk, SplitPlan};
use ferrite_av::transcode::Output;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Encode one slice of one source across every rung.
///
/// One decode per chunk feeds all of them, which is why a chunk is the unit
/// rather than a chunk-and-rung pair.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EncodeChunk {
    /// The source, reachable from the machine that runs this.
    pub input: PathBuf,
    /// Which slice.
    pub chunk: Chunk,
    /// One per rung.
    pub outputs: Vec<Output>,
}

/// Concatenate one rung's pieces back together.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JoinRung {
    /// Pieces in order.
    pub parts: Vec<PathBuf>,
    /// Where the rendition goes.
    pub output: PathBuf,
}

/// A whole asset, as handed to the workflow that runs it.
///
/// The plan is computed once and carried, not recomputed per worker: a retried
/// chunk has to reproduce identical boundaries.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssetJob {
    /// The source.
    pub input: PathBuf,
    /// Where everything goes.
    pub out_dir: PathBuf,
    /// Where the cuts land.
    pub plan: SplitPlan,
    /// One per rung, named and specified.
    pub rungs: Vec<Rung>,
}

/// One rung of the ladder.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Rung {
    /// Rendition name.
    pub name: String,
    /// How to encode it.
    pub spec: ferrite_av::EncodeSpec,
}

impl AssetJob {
    /// Where a rung's finished rendition goes.
    pub fn rendition(&self, rung: &Rung) -> PathBuf {
        self.out_dir
            .join("renditions")
            .join(format!("{}.mp4", rung.name))
    }

    /// Where one chunk of one rung goes before the join.
    pub fn part(&self, rung: &Rung, chunk: &Chunk) -> PathBuf {
        self.out_dir
            .join("renditions")
            .join("parts")
            .join(format!("{}-{:05}.mp4", rung.name, chunk.index))
    }

    /// The work for one chunk: every rung, one decode.
    pub fn encode_chunk(&self, chunk: &Chunk) -> EncodeChunk {
        EncodeChunk {
            input: self.input.clone(),
            chunk: *chunk,
            outputs: self
                .rungs
                .iter()
                .map(|r| Output {
                    path: self.part(r, chunk),
                    spec: r.spec.clone(),
                })
                .collect(),
        }
    }

    /// The work to reassemble one rung.
    pub fn join_rung(&self, rung: &Rung) -> JoinRung {
        JoinRung {
            parts: self
                .plan
                .chunks
                .iter()
                .map(|c| self.part(rung, c))
                .collect(),
            output: self.rendition(rung),
        }
    }

    /// Every chunk job, in order.
    pub fn chunks(&self) -> Vec<EncodeChunk> {
        self.plan
            .chunks
            .iter()
            .map(|c| self.encode_chunk(c))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferrite_av::encoder::{Preset, RateControl, Resolution, VideoCodecName};
    use ferrite_av::{EncodeSpec, Rational};

    fn job() -> AssetJob {
        let spec = |h: u32| {
            EncodeSpec::new(
                VideoCodecName::H264,
                Resolution::new(h * 16 / 9, h),
                Rational::new(30, 1),
            )
            .with_preset(Preset::Medium)
            .with_rate_control(RateControl::crf(23, 3_000_000))
        };
        AssetJob {
            input: PathBuf::from("/srv/source.mp4"),
            out_dir: PathBuf::from("/srv/asset"),
            plan: ferrite_av::split::plan(&[0, 10_000, 20_000, 30_000], 130_000, 10_000),
            rungs: vec![
                Rung {
                    name: "720p".into(),
                    spec: spec(720),
                },
                Rung {
                    name: "360p".into(),
                    spec: spec(360),
                },
            ],
        }
    }

    #[test]
    fn one_chunk_carries_every_rung() {
        // A chunk is the unit, not a chunk-and-rung pair: splitting them apart
        // would decode the same pictures once per rung.
        let job = job();
        let chunks = job.chunks();
        assert!(!chunks.is_empty());
        for chunk in &chunks {
            assert_eq!(chunk.outputs.len(), 2, "a chunk lost a rung");
        }
    }

    #[test]
    fn every_part_is_named_by_its_rung_and_position() {
        let job = job();
        let first = &job.chunks()[0];
        let names: Vec<String> = first
            .outputs
            .iter()
            .map(|o| o.path.file_name().unwrap().to_string_lossy().to_string())
            .collect();
        assert_eq!(names, ["720p-00000.mp4", "360p-00000.mp4"]);
    }

    #[test]
    fn a_join_collects_exactly_that_rungs_parts_in_order() {
        let job = job();
        let join = job.join_rung(&job.rungs[0]);

        assert_eq!(join.parts.len(), job.plan.chunks.len());
        assert!(
            join.parts
                .iter()
                .all(|p| p.to_string_lossy().contains("720p-"))
        );
        assert!(join.output.ends_with("720p.mp4"));

        let mut sorted = join.parts.clone();
        sorted.sort();
        assert_eq!(sorted, join.parts, "parts would join out of order");
    }

    #[test]
    fn the_whole_job_survives_the_trip_to_a_worker() {
        // The scheduler treats this as opaque bytes, so it has to round trip.
        let job = job();
        let json = serde_json::to_string(&job).unwrap();
        assert_eq!(serde_json::from_str::<AssetJob>(&json).unwrap(), job);
    }

    #[test]
    fn the_plan_travels_rather_than_being_recomputed() {
        // Split rule 6: a retried chunk must reproduce identical boundaries.
        let job = job();
        let carried: AssetJob =
            serde_json::from_str(&serde_json::to_string(&job).unwrap()).unwrap();
        assert_eq!(carried.plan, job.plan);
    }
}
