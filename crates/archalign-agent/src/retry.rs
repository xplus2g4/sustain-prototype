//! Retries for one completion (FR-GEN-08): HTTP 429, HTTP 5xx and timeouts are retried up to
//! 3 times, waiting 1 s, 2 s and 4 s, or `Retry-After` when the provider gave one. Each
//! attempt is bounded by the per-request timeout.

use std::time::Duration;

use crate::llm::{BoxFuture, LlmError};

/// Retry policy (Appendix C, Contract: 3 retries, 1 s / 2 s / 4 s).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetryPolicy {
    /// Waits before retry 1, 2 and 3.
    pub waits: Vec<Duration>,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            waits: vec![
                Duration::from_secs(1),
                Duration::from_secs(2),
                Duration::from_secs(4),
            ],
        }
    }
}

/// One retry, reported before waiting.
#[derive(Debug, Clone, PartialEq)]
pub struct RetryNotice {
    /// 1-based retry number.
    pub attempt: u32,
    /// HTTP status, when the failure had one.
    pub status: Option<u16>,
    /// Whether the failure was a timeout.
    pub timeout: bool,
    /// How long we wait before retrying.
    pub wait: Duration,
}

/// Something that can sleep (tokio in production, a recorder in tests).
pub trait Sleeper: Send + Sync {
    /// Sleeps for `d`.
    fn sleep(&self, d: Duration) -> BoxFuture<'_, ()>;
}

/// The real sleeper.
#[derive(Debug, Clone, Copy, Default)]
pub struct TokioSleeper;

impl Sleeper for TokioSleeper {
    fn sleep(&self, d: Duration) -> BoxFuture<'_, ()> {
        Box::pin(tokio::time::sleep(d))
    }
}

/// Runs `call` with the retry policy. Each attempt is wrapped in `request_timeout`; an
/// elapsed attempt counts as [`LlmError::Timeout`]. `on_retry` is told about each retry
/// before the wait. Returns the last error when retries run out or the error isn't
/// retryable.
pub async fn with_retries<'a, T, F>(
    policy: &RetryPolicy,
    request_timeout: Duration,
    sleeper: &dyn Sleeper,
    mut call: F,
    mut on_retry: impl FnMut(RetryNotice),
) -> Result<T, LlmError>
where
    F: FnMut() -> BoxFuture<'a, Result<T, LlmError>>,
{
    let mut attempt = 0usize;
    loop {
        let result = match tokio::time::timeout(request_timeout, call()).await {
            Ok(r) => r,
            Err(_) => Err(LlmError::Timeout),
        };
        let err = match result {
            Ok(v) => return Ok(v),
            Err(e) => e,
        };
        if !err.is_retryable() || attempt >= policy.waits.len() {
            return Err(err);
        }
        let default_wait = policy.waits[attempt];
        let (status, retry_after) = match &err {
            LlmError::Status {
                status,
                retry_after,
                ..
            } => (Some(*status), *retry_after),
            _ => (None, None),
        };
        let wait = retry_after.unwrap_or(default_wait);
        attempt += 1;
        on_retry(RetryNotice {
            attempt: attempt as u32,
            status,
            timeout: matches!(err, LlmError::Timeout),
            wait,
        });
        sleeper.sleep(wait).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    /// Runs a future on a current-thread runtime (no tokio `macros` feature needed).
    fn block_on<F: Future>(f: F) -> F::Output {
        tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .expect("runtime")
            .block_on(f)
    }

    #[derive(Default, Clone)]
    struct Recorder(Arc<Mutex<Vec<Duration>>>);

    impl Sleeper for Recorder {
        fn sleep(&self, d: Duration) -> BoxFuture<'_, ()> {
            self.0.lock().unwrap().push(d);
            Box::pin(async {})
        }
    }

    fn status(code: u16, retry_after: Option<u64>) -> LlmError {
        LlmError::Status {
            status: code,
            retry_after: retry_after.map(Duration::from_secs),
            message: "x".into(),
        }
    }

    /// A call that fails with the scripted errors, then succeeds.
    fn scripted(
        errors: Vec<LlmError>,
    ) -> impl FnMut() -> BoxFuture<'static, Result<u32, LlmError>> {
        let errors = Arc::new(Mutex::new(errors));
        let calls = Arc::new(Mutex::new(0u32));
        move || {
            let errors = errors.clone();
            let calls = calls.clone();
            Box::pin(async move {
                *calls.lock().unwrap() += 1;
                let mut e = errors.lock().unwrap();
                if e.is_empty() {
                    Ok(*calls.lock().unwrap())
                } else {
                    Err(e.remove(0))
                }
            })
        }
    }

    #[test]
    fn retries_429_5xx_and_timeouts_with_1_2_4_backoff() {
        block_on(async {
            let rec = Recorder::default();
            let mut notices = Vec::new();
            let r = with_retries(
                &RetryPolicy::default(),
                Duration::from_secs(300),
                &rec,
                scripted(vec![
                    status(429, None),
                    status(503, None),
                    LlmError::Timeout,
                ]),
                |n| notices.push(n),
            )
            .await;
            assert_eq!(r, Ok(4), "succeeds on the 4th call");
            assert_eq!(
                *rec.0.lock().unwrap(),
                vec![
                    Duration::from_secs(1),
                    Duration::from_secs(2),
                    Duration::from_secs(4)
                ]
            );
            assert_eq!(notices.len(), 3);
            assert_eq!(notices[0].status, Some(429));
            assert!(notices[2].timeout);
            assert_eq!(notices[2].attempt, 3);
        })
    }

    #[test]
    fn honors_retry_after() {
        block_on(async {
            let rec = Recorder::default();
            let r = with_retries(
                &RetryPolicy::default(),
                Duration::from_secs(300),
                &rec,
                scripted(vec![status(429, Some(7))]),
                |_| {},
            )
            .await;
            assert_eq!(r, Ok(2));
            assert_eq!(*rec.0.lock().unwrap(), vec![Duration::from_secs(7)]);
        })
    }

    #[test]
    fn gives_up_after_three_retries() {
        block_on(async {
            let rec = Recorder::default();
            let r = with_retries(
                &RetryPolicy::default(),
                Duration::from_secs(300),
                &rec,
                scripted(vec![status(500, None); 4]),
                |_| {},
            )
            .await;
            assert_eq!(r, Err(status(500, None)));
            assert_eq!(rec.0.lock().unwrap().len(), 3);
        })
    }

    #[test]
    fn does_not_retry_client_errors() {
        block_on(async {
            let rec = Recorder::default();
            let r = with_retries(
                &RetryPolicy::default(),
                Duration::from_secs(300),
                &rec,
                scripted(vec![status(401, None)]),
                |_| {},
            )
            .await;
            assert_eq!(r, Err(status(401, None)));
            assert!(rec.0.lock().unwrap().is_empty());
        })
    }

    #[test]
    fn an_elapsed_attempt_is_a_retryable_timeout() {
        block_on(async {
            let rec = Recorder::default();
            let slow_then_fast = {
                let n = Arc::new(Mutex::new(0));
                move || {
                    let n = n.clone();
                    Box::pin(async move {
                        let k = {
                            let mut g = n.lock().unwrap();
                            *g += 1;
                            *g
                        };
                        if k == 1 {
                            tokio::time::sleep(Duration::from_secs(60)).await;
                        }
                        Ok::<u32, LlmError>(k)
                    }) as BoxFuture<'static, Result<u32, LlmError>>
                }
            };
            let mut timeouts = 0;
            let r = with_retries(
                &RetryPolicy::default(),
                Duration::from_millis(20),
                &rec,
                slow_then_fast,
                |n| timeouts += n.timeout as u32,
            )
            .await;
            assert_eq!(r, Ok(2));
            assert_eq!(timeouts, 1);
        })
    }
}
