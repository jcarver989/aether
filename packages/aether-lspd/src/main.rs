use aether_lspd::run_daemon;
use clap::Parser;
use std::path::PathBuf;
use std::process::exit;
use std::time::Duration;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::writer::MakeWriterExt;

/// LSP Daemon for sharing language servers across agents
#[derive(Parser)]
#[command(name = "aether-lspd")]
#[command(about = "LSP daemon for sharing language servers across multiple agents")]
struct Args {
    /// Socket path to listen on
    #[arg(long)]
    socket: PathBuf,

    /// Idle timeout in seconds (0 = no timeout)
    #[arg(long, default_value = "300")]
    idle_timeout: u64,

    /// Log level (trace, debug, info, warn, error)
    #[arg(long, default_value = "info")]
    log_level: String,

    /// Log file path. If provided, logs are written to this file instead of stderr.
    /// Required for daemon mode since stderr is redirected to /dev/null.
    #[arg(long)]
    log_file: Option<PathBuf>,
}

fn main() {
    let args = Args::parse();

    if let Err(e) = daemonize() {
        eprintln!("Failed to daemonize: {e}");
        std::process::exit(1);
    }

    let runtime = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");

    runtime.block_on(async {
        let filter =
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(&args.log_level));

        if let Some(ref log_file) = args.log_file {
            // Ensure parent directory exists
            if let Some(parent) = log_file.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(log_file)
                .expect("Failed to open log file");

            tracing_subscriber::fmt()
                .with_env_filter(filter)
                .with_target(true)
                .with_ansi(false)
                .with_writer(file.with_max_level(tracing::Level::TRACE))
                .init();
        } else {
            tracing_subscriber::fmt()
                .with_env_filter(filter)
                .with_target(true)
                .init();
        }

        let idle_timeout = if args.idle_timeout == 0 {
            None
        } else {
            Some(Duration::from_secs(args.idle_timeout))
        };

        tracing::info!("Starting LSP daemon on socket: {:?}", args.socket);
        if let Err(e) = run_daemon(args.socket, idle_timeout).await {
            tracing::error!("Daemon error: {}", e);
            exit(1);
        }
    });
}

/// Daemonize the process (Unix only)
#[cfg(unix)]
fn daemonize() -> Result<(), String> {
    use nix::sys::signal::{SigHandler, Signal, signal};
    use nix::unistd::{ForkResult, fork, setsid};
    use std::fs::File;
    use std::os::unix::io::AsRawFd;

    match unsafe { fork() } {
        Ok(ForkResult::Parent { .. }) => exit(0),
        Ok(ForkResult::Child) => {}
        Err(e) => return Err(format!("First fork failed: {e}")),
    }

    setsid().map_err(|e| format!("setsid failed: {e}"))?;

    unsafe {
        signal(Signal::SIGHUP, SigHandler::SigIgn)
            .map_err(|e| format!("Failed to ignore SIGHUP: {e}"))?;
    }

    match unsafe { fork() } {
        Ok(ForkResult::Parent { .. }) => exit(0),
        Ok(ForkResult::Child) => {}
        Err(e) => return Err(format!("Second fork failed: {e}")),
    }

    let dev_null = File::open("/dev/null").map_err(|e| format!("Failed to open /dev/null: {e}"))?;
    let fd = dev_null.as_raw_fd();

    unsafe {
        if libc::dup2(fd, 0) == -1 {
            return Err(format!(
                "dup2 stdin failed: {}",
                std::io::Error::last_os_error()
            ));
        }
        if libc::dup2(fd, 1) == -1 {
            return Err(format!(
                "dup2 stdout failed: {}",
                std::io::Error::last_os_error()
            ));
        }
        if libc::dup2(fd, 2) == -1 {
            return Err(format!(
                "dup2 stderr failed: {}",
                std::io::Error::last_os_error()
            ));
        }
    }

    Ok(())
}

/// No-op daemonize for non-Unix platforms
#[cfg(not(unix))]
fn daemonize() -> Result<(), String> {
    Ok(())
}
