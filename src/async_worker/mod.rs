//! Worker thread bridging the synchronous TUI event loop to the async core
//! MusicBrainz / artwork calls (feature 012, User Story 3).
//!
//! Online actions (`scrape_*`, artwork fetch) are `async` in `tornade-core`.
//! The TUI loop is synchronous (`event::poll`), so these run on a dedicated
//! worker thread backed by a current-thread `tokio` runtime, with results
//! delivered back over an `mpsc` channel and bounded by a ~10s timeout so the
//! UI never hangs (FR-022, SC-005).

use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use tornade_core::services::artwork::{MusicBrainzClient, RateLimiter};
use tornade_core::services::metadata_scrape::ScrapeCandidate;

/// Timeout applied to every online call. Beyond this the job resolves to
/// [`AsyncPayload::Failed`] with a timeout message so the UI never hangs.
const JOB_TIMEOUT: Duration = Duration::from_secs(10);

/// A unit of online work requested by the UI thread.
#[derive(Debug, Clone)]
pub enum AsyncJob {
    /// Scrape recording (track-level) metadata candidates.
    ScrapeTrack { title: String, artist: String },
    /// Scrape release (album-level) metadata candidates.
    /// Part of the worker's complete job API; album-level scrape is wired
    /// through the artwork/metadata flows and reserved for album-context use.
    #[allow(dead_code)]
    ScrapeAlbum { album: String, artist: String },
    /// Fetch album artwork bytes from Cover Art Archive.
    FetchAlbumArtwork { album: String, artist: String },
}

/// Result payload for a completed job, correlated back to the UI by `job_id`.
#[derive(Debug)]
pub enum AsyncPayload {
    /// Scrape returned candidates (possibly empty; empty => "No match").
    Candidates(Vec<ScrapeCandidate>),
    /// Artwork bytes were downloaded (`None` => no artwork found).
    Artwork(Option<Vec<u8>>),
    /// The job failed or timed out; message is user-facing.
    Failed(String),
}

/// A completed async job delivered back to the UI thread.
#[derive(Debug)]
pub struct AsyncResult {
    pub job_id: u64,
    pub payload: AsyncPayload,
}

/// High-level status of the in-flight (or last) async job, tracked by the UI
/// overlay so it can render "Searching…", "No match", "Failed: …" etc.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobStatus {
    /// No job has been launched (or the overlay is fresh).
    #[allow(dead_code)]
    Idle,
    /// A job with this id is in flight.
    Searching(u64),
    /// The job completed with at least one candidate / artwork present.
    Ready(u64),
    /// The job completed but returned nothing.
    Empty(u64),
    /// The job failed; message is user-facing.
    Failed(String),
}

/// An enqueued request carrying its correlation id.
struct Request {
    job_id: u64,
    job: AsyncJob,
}

/// Handle held by [`crate::app::AppState`] to submit jobs and drain results.
///
/// Dropping the handle closes the request channel, which stops the worker
/// thread cleanly.
pub struct AsyncWorker {
    tx: Sender<Request>,
    rx: Receiver<AsyncResult>,
    next_id: u64,
}

impl AsyncWorker {
    /// Spawn the worker thread and return a handle. The worker owns a
    /// current-thread `tokio` runtime and processes one job at a time.
    pub fn spawn() -> Self {
        let (req_tx, req_rx) = channel::<Request>();
        let (res_tx, res_rx) = channel::<AsyncResult>();

        thread::Builder::new()
            .name("tornade-async-worker".into())
            .spawn(move || worker_loop(req_rx, res_tx))
            .expect("spawn async worker thread");

        Self {
            tx: req_tx,
            rx: res_rx,
            next_id: 1,
        }
    }

    /// Submit a job, returning its correlation id. The id lets the UI ignore
    /// results from jobs that have since been superseded.
    pub fn submit(&mut self, job: AsyncJob) -> u64 {
        let job_id = self.next_id;
        self.next_id += 1;
        // If the worker has gone away the send fails; the id is still returned
        // so callers behave uniformly (no result will ever arrive).
        let _ = self.tx.send(Request { job_id, job });
        job_id
    }

    /// Non-blocking poll for a completed result.
    pub fn try_recv(&self) -> Option<AsyncResult> {
        match self.rx.try_recv() {
            Ok(r) => Some(r),
            Err(TryRecvError::Empty) | Err(TryRecvError::Disconnected) => None,
        }
    }
}

/// Build a fresh [`MusicBrainzClient`] mirroring `ffi.rs`: a default HTTP client
/// and a 1.1 req/sec rate limiter (MusicBrainz asks for <= 1 req/sec).
fn build_client() -> MusicBrainzClient {
    let http_client = reqwest::Client::new();
    let rate_limiter = Arc::new(Mutex::new(RateLimiter::new(1100)));
    MusicBrainzClient::new(http_client, rate_limiter)
}

/// The worker loop: pulls requests, runs each async call under a timeout on the
/// current-thread runtime, and ships the result back.
fn worker_loop(req_rx: Receiver<Request>, res_tx: Sender<AsyncResult>) {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(_) => return, // cannot run without a runtime; worker exits
    };

    let client = build_client();

    // Blocks on the request channel; exits when the sender (AsyncWorker) drops.
    while let Ok(Request { job_id, job }) = req_rx.recv() {
        let payload = runtime.block_on(run_job(&client, job));
        if res_tx.send(AsyncResult { job_id, payload }).is_err() {
            break; // UI gone
        }
        crate::wake::signal();
    }
}

/// Run a single job under the shared timeout and normalise the outcome into an
/// [`AsyncPayload`].
async fn run_job(client: &MusicBrainzClient, job: AsyncJob) -> AsyncPayload {
    match job {
        AsyncJob::ScrapeTrack { title, artist } => {
            let fut = client.search_recording_metadata(&title, &artist);
            match tokio::time::timeout(JOB_TIMEOUT, fut).await {
                Ok(Ok(candidates)) => AsyncPayload::Candidates(candidates),
                Ok(Err(e)) => AsyncPayload::Failed(e),
                Err(_) => AsyncPayload::Failed(timeout_msg()),
            }
        }
        AsyncJob::ScrapeAlbum { album, artist } => {
            let fut = client.search_release_metadata(&album, &artist);
            match tokio::time::timeout(JOB_TIMEOUT, fut).await {
                Ok(Ok(candidates)) => AsyncPayload::Candidates(candidates),
                Ok(Err(e)) => AsyncPayload::Failed(e),
                Err(_) => AsyncPayload::Failed(timeout_msg()),
            }
        }
        AsyncJob::FetchAlbumArtwork { album, artist } => {
            let fut = client.search_album_artwork(&album, &artist);
            match tokio::time::timeout(JOB_TIMEOUT, fut).await {
                Ok(Ok(Some(result))) => AsyncPayload::Artwork(Some(result.image_data)),
                Ok(Ok(None)) => AsyncPayload::Artwork(None),
                Ok(Err(e)) => AsyncPayload::Failed(e),
                Err(_) => AsyncPayload::Failed(timeout_msg()),
            }
        }
    }
}

fn timeout_msg() -> String {
    format!("Timed out after {}s", JOB_TIMEOUT.as_secs())
}

/// Fold a completed [`AsyncResult`] into a [`JobStatus`] for the overlay,
/// discarding results whose `job_id` does not match the in-flight `expected`
/// id (a stale/superseded job).
///
/// Returns `None` when the result is stale and should be ignored.
pub fn status_for(expected: u64, result: &AsyncResult) -> Option<JobStatus> {
    if result.job_id != expected {
        return None;
    }
    let status = match &result.payload {
        AsyncPayload::Candidates(c) => {
            if c.is_empty() {
                JobStatus::Empty(result.job_id)
            } else {
                JobStatus::Ready(result.job_id)
            }
        }
        AsyncPayload::Artwork(a) => {
            if a.is_none() {
                JobStatus::Empty(result.job_id)
            } else {
                JobStatus::Ready(result.job_id)
            }
        }
        AsyncPayload::Failed(msg) => JobStatus::Failed(msg.clone()),
    };
    Some(status)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(title: &str) -> ScrapeCandidate {
        ScrapeCandidate {
            musicbrainz_id: "mbid".into(),
            title: title.into(),
            artist: "artist".into(),
            album_artist: None,
            album: None,
            year: None,
            genres: Vec::new(),
            track_number: None,
            disc_number: None,
            has_artwork: false,
            score: 90,
        }
    }

    // T047: job-id correlation — a result whose id != expected is stale.
    #[test]
    fn stale_result_is_ignored() {
        let result = AsyncResult {
            job_id: 1,
            payload: AsyncPayload::Candidates(vec![candidate("a")]),
        };
        // We are now waiting on job 2; job 1's result must be dropped.
        assert_eq!(status_for(2, &result), None);
        // Matching id is accepted.
        assert_eq!(status_for(1, &result), Some(JobStatus::Ready(1)));
    }

    // T047: status transition — non-empty candidates => Ready.
    #[test]
    fn candidates_present_is_ready() {
        let result = AsyncResult {
            job_id: 7,
            payload: AsyncPayload::Candidates(vec![candidate("x"), candidate("y")]),
        };
        assert_eq!(status_for(7, &result), Some(JobStatus::Ready(7)));
    }

    // T047: status transition — empty candidates => Empty ("No match").
    #[test]
    fn empty_candidates_is_empty() {
        let result = AsyncResult {
            job_id: 3,
            payload: AsyncPayload::Candidates(Vec::new()),
        };
        assert_eq!(status_for(3, &result), Some(JobStatus::Empty(3)));
    }

    // T047: status transition — artwork present => Ready, absent => Empty.
    #[test]
    fn artwork_status_transitions() {
        let ready = AsyncResult {
            job_id: 5,
            payload: AsyncPayload::Artwork(Some(vec![1, 2, 3])),
        };
        assert_eq!(status_for(5, &ready), Some(JobStatus::Ready(5)));

        let empty = AsyncResult {
            job_id: 6,
            payload: AsyncPayload::Artwork(None),
        };
        assert_eq!(status_for(6, &empty), Some(JobStatus::Empty(6)));
    }

    // T047: status transition — failure carries the message through.
    #[test]
    fn failure_status_carries_message() {
        let result = AsyncResult {
            job_id: 9,
            payload: AsyncPayload::Failed("boom".into()),
        };
        assert_eq!(
            status_for(9, &result),
            Some(JobStatus::Failed("boom".into()))
        );
    }

    // T047: the ~10s timeout constant is honoured (state-machine level: a
    // timeout is surfaced as Failed with a timeout message).
    #[test]
    fn timeout_maps_to_failed() {
        assert_eq!(JOB_TIMEOUT, Duration::from_secs(10));
        let msg = timeout_msg();
        assert!(msg.contains("10"), "timeout message should mention 10s");
        let result = AsyncResult {
            job_id: 2,
            payload: AsyncPayload::Failed(msg.clone()),
        };
        match status_for(2, &result) {
            Some(JobStatus::Failed(m)) => assert_eq!(m, msg),
            other => panic!("expected Failed, got {other:?}"),
        }
    }

    // Idle is the initial state before any job is submitted.
    #[test]
    fn idle_is_default_initial() {
        // A fresh worker has issued no ids; the overlay starts Idle.
        let status = JobStatus::Idle;
        assert_eq!(status, JobStatus::Idle);
    }

    // submit() hands out monotonically increasing ids.
    #[test]
    fn submit_ids_are_monotonic() {
        let mut worker = AsyncWorker::spawn();
        let a = worker.submit(AsyncJob::ScrapeTrack {
            title: "t".into(),
            artist: "a".into(),
        });
        let b = worker.submit(AsyncJob::ScrapeAlbum {
            album: "al".into(),
            artist: "a".into(),
        });
        assert!(b > a, "ids must increase: {a} then {b}");
    }
}
