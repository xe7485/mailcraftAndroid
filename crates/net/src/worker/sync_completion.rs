//! Completion signals for platform-triggered sync passes.

use super::{Job, Pool};
use std::collections::HashMap;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

#[derive(Clone, Default)]
pub(super) struct SyncCompletion(Arc<(Mutex<HashMap<u64, Progress>>, Condvar)>);

#[derive(Default)]
struct Progress {
    remaining: usize,
    success: bool,
}

/// Waits for every live account included in a platform sync request.
#[derive(Clone, Default)]
pub struct SyncWaiter(SyncCompletion);

impl SyncWaiter {
    /// Wait for all account passes. `None` means the request was stopped or timed out.
    pub fn wait_while(&self, request_id: u64, timeout: Duration, mut is_stopped: impl FnMut() -> bool) -> Option<bool> {
        let deadline = Instant::now().checked_add(timeout)?;
        let (progress, wake) = &*self.0.0;
        let mut progress = progress.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        loop {
            if is_stopped() {
                progress.remove(&request_id);
                return None;
            }
            let request = progress.get(&request_id)?;
            if request.remaining == 0 {
                let success = request.success;
                progress.remove(&request_id);
                return Some(success);
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                progress.remove(&request_id);
                return None;
            }
            progress = match wake.wait_timeout(progress, remaining.min(Duration::from_millis(500))) {
                Ok((progress, _)) => progress,
                Err(error) => error.into_inner().0,
            };
        }
    }
}

impl SyncCompletion {
    fn begin(&self, request_id: u64, accounts: usize) {
        let (progress, wake) = &*self.0;
        progress.lock().unwrap_or_else(std::sync::PoisonError::into_inner).insert(request_id, Progress { remaining: accounts, success: true });
        wake.notify_all();
    }

    pub(super) fn finish(&self, request_id: u64, success: bool) {
        let (progress, wake) = &*self.0;
        let mut progress = progress.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(request) = progress.get_mut(&request_id)
            && request.remaining > 0
        {
            request.remaining -= 1;
            request.success &= success;
            wake.notify_all();
        }
    }

    pub(super) fn cancel_all(&self) {
        let (progress, wake) = &*self.0;
        let mut progress = progress.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        for request in progress.values_mut() {
            request.remaining = 0;
            request.success = false;
        }
        wake.notify_all();
    }
}

impl Pool {
    /// Request one complete sync pass from each remote account and return its waiter.
    pub fn request_full_sync(&mut self, request_id: u64) -> SyncWaiter {
        let accounts: Vec<String> = self.configs.iter().filter(|(_, config)| !config.is_local()).map(|(id, _)| id.clone()).collect();
        self.sync_completion.begin(request_id, accounts.len());
        for account in accounts {
            self.submit(Job::SyncAllTracked { account, request_id });
        }
        SyncWaiter(self.sync_completion.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::{SyncCompletion, SyncWaiter};
    use std::time::Duration;

    #[test]
    fn completes_after_every_account_and_keeps_failures() {
        let completion = SyncCompletion::default();
        completion.begin(41, 2);
        let waiter = SyncWaiter(completion.clone());
        completion.finish(41, true);
        assert_eq!(waiter.wait_while(41, Duration::from_millis(1), || false), None);

        completion.begin(42, 2);
        completion.finish(42, false);
        completion.finish(42, true);
        assert_eq!(waiter.wait_while(42, Duration::from_secs(1), || false), Some(false));
    }

    #[test]
    fn completes_empty_requests_immediately() {
        let completion = SyncCompletion::default();
        completion.begin(7, 0);
        assert_eq!(SyncWaiter(completion).wait_while(7, Duration::from_secs(1), || false), Some(true));
    }
}
