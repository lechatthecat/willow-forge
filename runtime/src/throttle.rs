//! Fixed-window rate limiter backed by Redis.
//!
//! Counts hits against a key inside a sliding fixed window and reports when the
//! limit is exceeded. Used to throttle abuse-prone, mail-sending endpoints
//! (password-reset request, verification resend) so they cannot be used to
//! bomb a victim's inbox.
//!
//! ```rust,ignore
//! let key = format!("throttle:forgot-password:{}", email);
//! if Throttle::too_many(&ctx, &key, 5, 60).await? {
//!     // 6th attempt within 60s — refuse to send.
//! }
//! ```

use redis::AsyncCommands;

use crate::app_errors::AppError;
use crate::context::Context;

pub struct Throttle;

impl Throttle {
    /// Record one hit against `key` and report whether the caller has now
    /// exceeded `max` hits within `window_secs`.
    ///
    /// The window starts on the first hit (when the counter is created) and the
    /// key expires after `window_secs`, so counts reset automatically. Returns
    /// `true` once the hit count is greater than `max`.
    pub async fn too_many(
        ctx: &Context,
        key: &str,
        max: u64,
        window_secs: i64,
    ) -> Result<bool, AppError> {
        let mut conn = ctx.state.services.redis.get_async_connection().await?;
        let count: u64 = conn.incr(key, 1).await?;
        if count == 1 {
            // First hit in this window — arm the expiry so the counter resets.
            let _: () = conn.expire(key, window_secs).await?;
        }
        Ok(count > max)
    }
}
