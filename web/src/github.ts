/// The single public GitHub endpoint this page reads, and how it is read.

export const GITHUB_REPOSITORY_API = "https://api.github.com/repos/viniciusnevescosta/normfix";

/// Request options for the repository metadata request.
///
/// The cache mode is the load-bearing part. `force-cache` serves a stored
/// response regardless of its age, so a star count would freeze at whatever the
/// browser happened to see first and never change again for that reader.
/// GitHub sends `max-age=60` with an ETag, so the default mode is both fresher
/// and cheap: it reuses the response for a minute, then revalidates
/// conditionally instead of refetching the body.
export function githubRequestInit(): RequestInit {
  return {
    cache: "default",
    credentials: "omit",
    referrerPolicy: "no-referrer",
  };
}

/// Reads a star count out of a repository payload.
///
/// The value is rendered next to a link, so anything that is not a plain
/// non-negative count is rejected rather than displayed.
export function starCount(payload: unknown): number | null {
  if (typeof payload !== "object" || payload === null) return null;
  const value = (payload as { stargazers_count?: unknown }).stargazers_count;
  return typeof value === "number" && Number.isInteger(value) && value >= 0 ? value : null;
}
