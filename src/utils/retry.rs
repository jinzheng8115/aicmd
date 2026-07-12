use anyhow::Error;
use parking_lot::Mutex;
use std::{
    future::Future,
    sync::Arc,
    time::{Duration, Instant},
};

use super::{
    abortable_run_with_progress, abortable_run_with_spinner, format_progress_text, is_chinese,
    localized, AbortSignal, IS_STDOUT_TERMINAL,
};

pub const EXTERNAL_ATTEMPT_TIMEOUT: Duration = Duration::from_secs(15);
pub const EXTERNAL_TOTAL_TIMEOUT: Duration = Duration::from_secs(45);
pub const EXTERNAL_MAX_ATTEMPTS: usize = 3;

#[derive(Debug, Clone, Copy)]
pub struct ProgressStage<'a> {
    pub zh: &'a str,
    pub en: &'a str,
}

impl<'a> ProgressStage<'a> {
    pub const fn new(zh: &'a str, en: &'a str) -> Self {
        Self { zh, en }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct RetryPolicy {
    pub attempt_timeout: Duration,
    pub total_timeout: Duration,
    pub max_attempts: usize,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            attempt_timeout: EXTERNAL_ATTEMPT_TIMEOUT,
            total_timeout: EXTERNAL_TOTAL_TIMEOUT,
            max_attempts: EXTERNAL_MAX_ATTEMPTS,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetryAttempt {
    pub number: usize,
    pub max_attempts: usize,
    pub timeout: Duration,
}

#[derive(Debug)]
struct RetryState {
    started_at: Instant,
    attempts: usize,
}

#[derive(Debug, Clone)]
pub struct RetryBudget {
    policy: RetryPolicy,
    state: Arc<Mutex<RetryState>>,
}

impl Default for RetryBudget {
    fn default() -> Self {
        Self::with_policy(RetryPolicy::default())
    }
}

impl RetryBudget {
    pub fn with_policy(policy: RetryPolicy) -> Self {
        Self {
            policy,
            state: Arc::new(Mutex::new(RetryState {
                started_at: Instant::now(),
                attempts: 0,
            })),
        }
    }

    pub fn begin_attempt(&self) -> Option<RetryAttempt> {
        let mut state = self.state.lock();
        if state.attempts >= self.policy.max_attempts {
            return None;
        }
        let remaining = self
            .policy
            .total_timeout
            .checked_sub(state.started_at.elapsed())?;
        if remaining.is_zero() {
            return None;
        }
        state.attempts += 1;
        Some(RetryAttempt {
            number: state.attempts,
            max_attempts: self.policy.max_attempts,
            timeout: remaining.min(self.policy.attempt_timeout),
        })
    }

    fn attempts(&self) -> usize {
        self.state.lock().attempts
    }
}

pub fn is_retryable_external_error(error: &Error) -> bool {
    let text = format!("{error:#}").to_ascii_lowercase();
    let fatal = [
        "http status 400",
        "http status 401",
        "http status 403",
        "http status 404",
        "api key",
        "new_sensitive",
        "tool selection",
        "invalid execution plan",
    ];
    if fatal.iter().any(|term| text.contains(term)) {
        return false;
    }
    [
        "timed out",
        "timeout",
        "connection reset",
        "connection refused",
        "connection closed",
        "connection error",
        "transport interrupted",
        "error sending request",
        "channel disconnected",
        "server exited",
        "http status 429",
        "http status 500",
        "http status 502",
        "http status 503",
        "http status 504",
    ]
    .iter()
    .any(|term| text.contains(term))
}

fn is_timeout_error(error: &Error) -> bool {
    let text = format!("{error:#}").to_ascii_lowercase();
    text.contains("timed out") || text.contains("timeout")
}

fn retry_notice(attempt: usize, max_attempts: usize, chinese: bool, error: &Error) -> String {
    if is_timeout_error(error) {
        if chinese {
            format!("第 {attempt}/{max_attempts} 次超时，正在重试")
        } else {
            format!("Attempt {attempt}/{max_attempts} timed out; retrying")
        }
    } else if chinese {
        format!("第 {attempt}/{max_attempts} 次失败，正在重试")
    } else {
        format!("Attempt {attempt}/{max_attempts} failed; retrying")
    }
}

fn retry_stopped_message(budget: &RetryBudget, chinese: bool) -> String {
    if budget.attempts() >= budget.policy.max_attempts {
        localized(
            "已尝试 3 次，仍未获得响应，操作已中断",
            "No response after 3 attempts; operation stopped",
        )
        .to_string()
    } else if chinese {
        format!(
            "已达到 {} 秒总时间上限，操作已中断",
            budget.policy.total_timeout.as_secs()
        )
    } else {
        format!(
            "Reached the {}s total time limit; operation stopped",
            budget.policy.total_timeout.as_secs()
        )
    }
}

pub async fn run_external_with_retry<F, Fut, T>(
    stage: ProgressStage<'_>,
    budget: &RetryBudget,
    abort_signal: AbortSignal,
    task_factory: F,
) -> anyhow::Result<T>
where
    F: FnMut(RetryAttempt) -> Fut,
    Fut: Future<Output = anyhow::Result<T>>,
{
    run_external_with_retry_inner(stage, budget, abort_signal, true, true, task_factory).await
}

pub async fn run_external_with_managed_retry<F, Fut, T>(
    stage: ProgressStage<'_>,
    budget: &RetryBudget,
    abort_signal: AbortSignal,
    task_factory: F,
) -> anyhow::Result<T>
where
    F: FnMut(RetryAttempt) -> Fut,
    Fut: Future<Output = anyhow::Result<T>>,
{
    run_external_with_retry_inner(stage, budget, abort_signal, false, true, task_factory).await
}

pub async fn run_external_with_retry_quiet<F, Fut, T>(
    stage: ProgressStage<'_>,
    budget: &RetryBudget,
    abort_signal: AbortSignal,
    task_factory: F,
) -> anyhow::Result<T>
where
    F: FnMut(RetryAttempt) -> Fut,
    Fut: Future<Output = anyhow::Result<T>>,
{
    run_external_with_retry_inner(stage, budget, abort_signal, true, false, task_factory).await
}

async fn run_external_with_retry_inner<F, Fut, T>(
    stage: ProgressStage<'_>,
    budget: &RetryBudget,
    abort_signal: AbortSignal,
    enforce_timeout: bool,
    show_progress: bool,
    mut task_factory: F,
) -> anyhow::Result<T>
where
    F: FnMut(RetryAttempt) -> Fut,
    Fut: Future<Output = anyhow::Result<T>>,
{
    let mut last_error: Option<anyhow::Error> = None;
    loop {
        let Some(attempt) = budget.begin_attempt() else {
            let message = retry_stopped_message(budget, is_chinese());
            return match last_error {
                Some(error) => Err(error.context(message)),
                None => Err(anyhow::anyhow!(message)),
            };
        };
        let chinese = is_chinese();
        let stage = if chinese { stage.zh } else { stage.en };
        let status = format_progress_text(stage, attempt.number, attempt.max_attempts, 0, chinese);
        if !*IS_STDOUT_TERMINAL || !show_progress {
            eprintln!("{status}");
        }
        let task = task_factory(attempt);
        let attempt_task = async move {
            if !enforce_timeout {
                return task.await;
            }
            tokio::time::timeout(attempt.timeout, task)
                .await
                .map_err(|_| anyhow::anyhow!("{stage} timed out"))?
        };
        let result = if show_progress {
            abortable_run_with_progress(
                attempt_task,
                stage,
                attempt.number,
                attempt.max_attempts,
                chinese,
                abort_signal.clone(),
            )
            .await
        } else {
            abortable_run_with_spinner(attempt_task, "", abort_signal.clone()).await
        };

        match result {
            Ok(value) => return Ok(value),
            Err(error) if abort_signal.aborted() => return Err(error),
            Err(error)
                if attempt.number < attempt.max_attempts && is_retryable_external_error(&error) =>
            {
                eprintln!(
                    "{}",
                    retry_notice(attempt.number, attempt.max_attempts, chinese, &error)
                );
                last_error = Some(error);
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
            Err(error) if is_retryable_external_error(&error) => {
                return Err(error.context(localized(
                    "已尝试 3 次，仍未获得响应，操作已中断",
                    "No response after 3 attempts; operation stopped",
                )));
            }
            Err(error) => return Err(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::create_abort_signal;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    #[test]
    fn retry_budget_allows_three_attempts_with_fifteen_second_slices() {
        let budget = RetryBudget::with_policy(RetryPolicy {
            attempt_timeout: Duration::from_secs(15),
            total_timeout: Duration::from_secs(45),
            max_attempts: 3,
        });

        assert_eq!(budget.begin_attempt().unwrap().number, 1);
        assert_eq!(budget.begin_attempt().unwrap().number, 2);
        assert_eq!(budget.begin_attempt().unwrap().number, 3);
        assert!(budget.begin_attempt().is_none());
    }

    #[test]
    fn retryable_errors_are_limited_to_transient_failures() {
        for message in [
            "connection reset by peer",
            "connection error: transport interrupted",
            "error sending request for url",
            "request timed out",
            "HTTP status 429",
            "HTTP status 500",
            "HTTP status 502",
            "HTTP status 503",
            "HTTP status 504",
            "MCP response channel disconnected",
        ] {
            assert!(
                is_retryable_external_error(&anyhow::anyhow!(message)),
                "{message}"
            );
        }
        for message in [
            "HTTP status 400",
            "HTTP status 401",
            "HTTP status 403",
            "HTTP status 404",
            "invalid api key",
            "new_sensitive (type: api_error)",
            "tool selection failed",
        ] {
            assert!(
                !is_retryable_external_error(&anyhow::anyhow!(message)),
                "{message}"
            );
        }
    }

    #[test]
    fn retry_notice_calls_out_a_timeout() {
        let message = retry_notice(1, 3, true, &anyhow::anyhow!("request timed out"));

        assert_eq!(message, "第 1/3 次超时，正在重试");
    }

    #[tokio::test]
    async fn external_call_retries_twice_then_succeeds() {
        let attempts = AtomicUsize::new(0);
        let budget = RetryBudget::with_policy(RetryPolicy {
            attempt_timeout: Duration::from_millis(50),
            total_timeout: Duration::from_secs(2),
            max_attempts: 3,
        });
        let value = run_external_with_retry(
            ProgressStage::new("测试阶段", "Test stage"),
            &budget,
            create_abort_signal(),
            |_| async {
                let attempt = attempts.fetch_add(1, Ordering::SeqCst) + 1;
                if attempt < 3 {
                    anyhow::bail!("connection reset by peer");
                }
                Ok("done")
            },
        )
        .await
        .unwrap();

        assert_eq!(value, "done");
        assert_eq!(attempts.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn external_call_does_not_retry_fatal_errors() {
        let attempts = AtomicUsize::new(0);
        let budget = RetryBudget::with_policy(RetryPolicy {
            attempt_timeout: Duration::from_millis(50),
            total_timeout: Duration::from_secs(2),
            max_attempts: 3,
        });
        let error = run_external_with_retry(
            ProgressStage::new("测试阶段", "Test stage"),
            &budget,
            create_abort_signal(),
            |_| async {
                attempts.fetch_add(1, Ordering::SeqCst);
                Err::<(), _>(anyhow::anyhow!("HTTP status 401"))
            },
        )
        .await
        .unwrap_err();

        assert!(error.to_string().contains("401"));
        assert_eq!(attempts.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn third_transient_failure_reports_limit_and_last_error() {
        let budget = RetryBudget::with_policy(RetryPolicy {
            attempt_timeout: Duration::from_millis(50),
            total_timeout: Duration::from_secs(2),
            max_attempts: 3,
        });
        let error = run_external_with_retry(
            ProgressStage::new("测试阶段", "Test stage"),
            &budget,
            create_abort_signal(),
            |_| async { Err::<(), _>(anyhow::anyhow!("connection reset by peer")) },
        )
        .await
        .unwrap_err();
        let display = format!("{error:#}");

        assert!(display.contains("已尝试 3 次") || display.contains("3 attempts"));
        assert!(display.contains("connection reset by peer"));
    }
}
