//! ArchAlign: native viewer and generator for Archify architecture diagrams.
//! Unofficial; not affiliated with Archify. Includes material derived from Archify (MIT).

fn main() -> iced::Result {
    // Logs go to stderr only, filtered by RUST_LOG (FR-SEC-06).
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    archalign_desktop::run(archalign_desktop::persist::boot_from_disk())
}
