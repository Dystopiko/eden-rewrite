use std::path::Path;

/// Checks whether this program is running in Docker, Podman or Kubernetes.
#[must_use]
pub fn is_running_in_container() -> bool {
    static CANDIDATES: &[&str] = &[
        "/.dockerenv",
        "/run/.dockerenv",
        "/run/secrets/kubernetes.io",
        "/var/run/secrets/kubernetes.io",
    ];

    // read /proc/1/sched first to really perform we're in the container
    let is_container_in_scheduler = eden_paths::read(Path::new("/proc/1/sched"))
        .map(|v| v.contains("docker") || v.contains("/lxc"))
        .unwrap_or(false);

    is_container_in_scheduler || CANDIDATES.iter().map(Path::new).any(Path::exists)
}
