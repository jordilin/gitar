// Limits the number of pages we can pull from the remote.
// Default number or results per_page:
// Github 30
// Gitlab 20
// Max number of pages to pull from the remote.
pub const REST_API_MAX_PAGES: u32 = 10;

// Number of requests remaining threshold. If we reach, we stop for precaution
// before we reach 0.
pub const RATE_LIMIT_REMAINING_THRESHOLD: u32 = 10;
