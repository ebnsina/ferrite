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
    /// The rungs already on disk, so a republish can include whatever has
    /// landed rather than waiting for the whole ladder.
    pub fn ready_rungs(&self) -> Vec<&Rung> {
        self.rungs
            .iter()
            .filter(|r| self.rendition(r).is_file())
            .collect()
    }

    /// Split into the rung that makes the asset playable and the rest.
    ///
    /// The fast path is one mid rung: high enough to watch, small enough to
    /// finish. Latency beats efficiency for exactly one rendition.
    pub fn two_paths(&self) -> (AssetJob, AssetJob) {
        let middle = self.rungs.len() / 2;
        let fast = AssetJob {
            rungs: self.rungs.get(middle).cloned().into_iter().collect(),
            ..self.clone()
        };
        let quality = AssetJob {
            rungs: self
                .rungs
                .iter()
                .enumerate()
                .filter(|(i, _)| *i != middle)
                .map(|(_, r)| r.clone())
                .collect(),
            ..self.clone()
        };
        (fast, quality)
    }

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
    fn the_fast_path_takes_one_middle_rung_and_leaves_the_rest() {
        let job = job();
        let (fast, quality) = job.two_paths();

        assert_eq!(fast.rungs.len(), 1, "the fast path is one rung");
        assert_eq!(quality.rungs.len(), job.rungs.len() - 1);

        // Between them they cover the ladder exactly once.
        let mut names: Vec<&str> = fast
            .rungs
            .iter()
            .chain(&quality.rungs)
            .map(|r| r.name.as_str())
            .collect();
        names.sort_unstable();
        let mut all: Vec<&str> = job.rungs.iter().map(|r| r.name.as_str()).collect();
        all.sort_unstable();
        assert_eq!(names, all);
    }

    #[test]
    fn both_paths_cut_at_the_same_places() {
        // They land in one manifest, so a player switching between a fast rung
        // and a quality one needs identical boundaries.
        let job = job();
        let (fast, quality) = job.two_paths();
        assert_eq!(fast.plan, job.plan);
        assert_eq!(quality.plan, job.plan);
        assert_eq!(fast.out_dir, quality.out_dir);
    }

    #[test]
    fn a_single_rung_ladder_leaves_the_quality_path_empty() {
        let mut job = job();
        job.rungs.truncate(1);
        let (fast, quality) = job.two_paths();
        assert_eq!(fast.rungs.len(), 1);
        assert!(
            quality.rungs.is_empty(),
            "there is nothing left to follow up with"
        );
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
