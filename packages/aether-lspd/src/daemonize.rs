use std::process::exit;

#[cfg(unix)]
pub fn daemonize() -> Result<(), String> {
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
        signal(Signal::SIGHUP, SigHandler::SigIgn).map_err(|e| format!("Failed to ignore SIGHUP: {e}"))?;
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
            return Err(format!("dup2 stdin failed: {}", std::io::Error::last_os_error()));
        }
        if libc::dup2(fd, 1) == -1 {
            return Err(format!("dup2 stdout failed: {}", std::io::Error::last_os_error()));
        }
        if libc::dup2(fd, 2) == -1 {
            return Err(format!("dup2 stderr failed: {}", std::io::Error::last_os_error()));
        }
    }

    Ok(())
}

#[cfg(not(unix))]
pub fn daemonize() -> Result<(), String> {
    Ok(())
}
