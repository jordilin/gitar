// Limits the number of pages we can pull from the remote.
// Default number or results per_page:
// Github 30
// Gitlab 20
// Max number of pages to pull from the remote.
pub const REST_API_MAX_PAGES: u32 = 10;

// Number of requests remaining threshold. If we reach, we stop for precaution
// before we reach 0.
pub const RATE_LIMIT_REMAINING_THRESHOLD: u32 = 10;

// most limiting Github 5000/60 = 83.33 requests per minute. Round
// up to 80.
pub const DEFAULT_NUMBER_REQUESTS_MINUTE: u32 = 80;

// Default number of results per page for --num-resources. Gitlab 20, Github 30
// As this is an approximation, we will use 30 if per_page is not provided.
pub const DEFAULT_PER_PAGE: u32 = 30;
