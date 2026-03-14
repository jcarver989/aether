use std::env;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

const SANDBOX_IMAGE: &str = "aether-sandbox:latest";

const FORWARDED_KEYS: &[&str] = &[
    "ANTHROPIC_API_KEY",
    "OPENROUTER_API_KEY",
    "OPENAI_API_KEY",
    "OLLAMA_HOST",
];

const AETHER_ENV_PREFIX: &str = "AETHER_";

#[derive(Debug)]
pub enum SandboxError {
    DockerNotFound,
    DockerNotRunning(String),
    ImageNotFound(String),
    ExecFailed(io::Error),
    HomeNotResolvable,
}

impl fmt::Display for SandboxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SandboxError::DockerNotFound => {
                write!(f, "Docker is not installed or not in PATH")
            }
            SandboxError::DockerNotRunning(msg) => {
                write!(f, "Docker daemon is not running: {msg}")
            }
            SandboxError::ImageNotFound(image) => {
                write!(
                    f,
                    "Sandbox image '{image}' not found. Build it with:\n\
                     cargo build --release -p aether-cli\n\
                     cp target/release/aether docker/\n\
                     docker build -t {image} -f docker/Dockerfile.sandbox docker/"
                )
            }
            SandboxError::ExecFailed(err) => write!(f, "Failed to exec docker: {err}"),
            SandboxError::HomeNotResolvable => {
                write!(f, "Could not determine home directory")
            }
        }
    }
}

impl std::error::Error for SandboxError {}

/// Entry point called from `main()` when `--sandbox` flag is present.
pub fn exec_in_container() -> ExitCode {
    match try_exec_in_container() {
        Ok(code) => code,
        Err(err) => {
            eprintln!("Sandbox error: {err}");
            ExitCode::FAILURE
        }
    }
}

fn try_exec_in_container() -> Result<ExitCode, SandboxError> {
    check_docker()?;
    check_image(SANDBOX_IMAGE)?;

    let cwd = env::current_dir().map_err(SandboxError::ExecFailed)?;
    let aether_home = resolve_aether_home()?;
    let args: Vec<String> = env::args().collect();
    let inner_args = filter_sandbox_flag(&args);
    let env_vars = select_forwarded_vars(env::vars());

    let docker_args = build_docker_args(&cwd, &aether_home, &env_vars, &inner_args);

    exec_docker(&docker_args)
}

fn check_docker() -> Result<(), SandboxError> {
    let output = Command::new("docker")
        .arg("info")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .output()
        .map_err(|_| SandboxError::DockerNotFound)?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        return Err(SandboxError::DockerNotRunning(stderr));
    }

    Ok(())
}

fn check_image(image: &str) -> Result<(), SandboxError> {
    let output = Command::new("docker")
        .args(["image", "inspect", image])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .map_err(|_| SandboxError::DockerNotFound)?;

    if !output.status.success() {
        return Err(SandboxError::ImageNotFound(image.to_string()));
    }

    Ok(())
}

fn resolve_aether_home() -> Result<PathBuf, SandboxError> {
    if let Ok(val) = env::var("AETHER_HOME") {
        return Ok(PathBuf::from(val));
    }
    let home = dirs::home_dir().ok_or(SandboxError::HomeNotResolvable)?;
    Ok(home.join(".aether"))
}

fn filter_sandbox_flag(args: &[String]) -> Vec<String> {
    args.iter()
        .filter(|a| *a != "--sandbox")
        .cloned()
        .collect()
}

fn select_forwarded_vars(
    vars: impl Iterator<Item = (String, String)>,
) -> Vec<(String, String)> {
    vars.filter(|(key, _)| {
        FORWARDED_KEYS.contains(&key.as_str()) || key.starts_with(AETHER_ENV_PREFIX)
    })
    .collect()
}

fn build_docker_args(
    cwd: &Path,
    aether_home: &Path,
    env_vars: &[(String, String)],
    inner_args: &[String],
) -> Vec<String> {
    let mut args = vec![
        "run".to_string(),
        "--rm".to_string(),
        "-i".to_string(),
        "--network".to_string(),
        "host".to_string(),
        "-w".to_string(),
        "/workspace".to_string(),
        "-v".to_string(),
        format!("{}:/workspace", cwd.display()),
        "-v".to_string(),
        format!("{}:/root/.aether", aether_home.display()),
        "-e".to_string(),
        "AETHER_HOME=/root/.aether".to_string(),
        "-e".to_string(),
        "AETHER_INSIDE_SANDBOX=1".to_string(),
    ];

    for (key, value) in env_vars {
        args.push("-e".to_string());
        args.push(format!("{key}={value}"));
    }

    args.push(SANDBOX_IMAGE.to_string());

    // Skip the binary name (first element) — the ENTRYPOINT already provides it
    if inner_args.len() > 1 {
        args.extend(inner_args[1..].iter().cloned());
    }

    args
}

#[cfg(unix)]
fn exec_docker(args: &[String]) -> Result<ExitCode, SandboxError> {
    use std::os::unix::process::CommandExt;

    let err = Command::new("docker").args(args).exec();
    Err(SandboxError::ExecFailed(err))
}

#[cfg(not(unix))]
fn exec_docker(args: &[String]) -> Result<ExitCode, SandboxError> {
    let status = Command::new("docker")
        .args(args)
        .status()
        .map_err(SandboxError::ExecFailed)?;

    Ok(match status.code() {
        Some(0) => ExitCode::SUCCESS,
        _ => ExitCode::FAILURE,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_sandbox_flag_strips_flag() {
        let args = vec![
            "aether".to_string(),
            "--sandbox".to_string(),
            "headless".to_string(),
            "-m".to_string(),
            "gpt-4".to_string(),
        ];
        let filtered = filter_sandbox_flag(&args);
        assert_eq!(filtered, vec!["aether", "headless", "-m", "gpt-4"]);
    }

    #[test]
    fn filter_sandbox_flag_middle_position() {
        let args = vec![
            "aether".to_string(),
            "headless".to_string(),
            "--sandbox".to_string(),
            "-m".to_string(),
        ];
        let filtered = filter_sandbox_flag(&args);
        assert_eq!(filtered, vec!["aether", "headless", "-m"]);
    }

    #[test]
    fn filter_sandbox_flag_noop_when_absent() {
        let args = vec![
            "aether".to_string(),
            "headless".to_string(),
            "-m".to_string(),
        ];
        let filtered = filter_sandbox_flag(&args);
        assert_eq!(filtered, args);
    }

    #[test]
    fn filter_sandbox_flag_strips_multiple() {
        let args = vec![
            "aether".to_string(),
            "--sandbox".to_string(),
            "headless".to_string(),
            "--sandbox".to_string(),
        ];
        let filtered = filter_sandbox_flag(&args);
        assert_eq!(filtered, vec!["aether", "headless"]);
    }

    #[test]
    fn select_forwarded_vars_includes_known_keys() {
        let vars = vec![
            ("ANTHROPIC_API_KEY".to_string(), "sk-123".to_string()),
            ("OPENROUTER_API_KEY".to_string(), "or-456".to_string()),
            ("HOME".to_string(), "/root".to_string()),
            ("PATH".to_string(), "/usr/bin".to_string()),
        ];
        let forwarded = select_forwarded_vars(vars.into_iter());
        assert_eq!(forwarded.len(), 2);
        assert!(forwarded.iter().any(|(k, _)| k == "ANTHROPIC_API_KEY"));
        assert!(forwarded.iter().any(|(k, _)| k == "OPENROUTER_API_KEY"));
    }

    #[test]
    fn select_forwarded_vars_includes_aether_prefix() {
        let vars = vec![
            ("AETHER_DEBUG".to_string(), "1".to_string()),
            ("AETHER_LOG_LEVEL".to_string(), "trace".to_string()),
            ("SOMETHING_ELSE".to_string(), "nope".to_string()),
        ];
        let forwarded = select_forwarded_vars(vars.into_iter());
        assert_eq!(forwarded.len(), 2);
        assert!(forwarded.iter().any(|(k, _)| k == "AETHER_DEBUG"));
        assert!(forwarded.iter().any(|(k, _)| k == "AETHER_LOG_LEVEL"));
    }

    #[test]
    fn select_forwarded_vars_excludes_unknown() {
        let vars = vec![
            ("HOME".to_string(), "/root".to_string()),
            ("EDITOR".to_string(), "vim".to_string()),
        ];
        let forwarded = select_forwarded_vars(vars.into_iter());
        assert!(forwarded.is_empty());
    }

    #[test]
    fn build_docker_args_contains_expected_flags() {
        let cwd = Path::new("/home/user/project");
        let aether_home = Path::new("/home/user/.aether");
        let env_vars = vec![("ANTHROPIC_API_KEY".to_string(), "sk-123".to_string())];
        let inner_args = vec![
            "aether".to_string(),
            "headless".to_string(),
            "-m".to_string(),
            "gpt-4".to_string(),
        ];

        let args = build_docker_args(cwd, aether_home, &env_vars, &inner_args);

        assert!(args.contains(&"run".to_string()));
        assert!(args.contains(&"--rm".to_string()));
        assert!(args.contains(&"-i".to_string()));
        assert!(args.contains(&"--network".to_string()));
        assert!(args.contains(&"host".to_string()));
        assert!(args.contains(&"/workspace".to_string()));
        assert!(args.contains(&format!("{}:/workspace", cwd.display())));
        assert!(args.contains(&format!("{}:/root/.aether", aether_home.display())));
        assert!(args.contains(&"AETHER_HOME=/root/.aether".to_string()));
        assert!(args.contains(&"AETHER_INSIDE_SANDBOX=1".to_string()));
        assert!(args.contains(&"ANTHROPIC_API_KEY=sk-123".to_string()));
        assert!(args.contains(&SANDBOX_IMAGE.to_string()));
        // Inner args skip the binary name
        assert!(args.contains(&"headless".to_string()));
        assert!(args.contains(&"-m".to_string()));
        assert!(args.contains(&"gpt-4".to_string()));
        // Binary name must NOT appear after the image
        let image_pos = args.iter().position(|a| a == SANDBOX_IMAGE).unwrap();
        assert!(!args[image_pos..].contains(&"aether".to_string()));
    }

    #[test]
    fn build_docker_args_skips_binary_name_only() {
        let cwd = Path::new("/tmp");
        let aether_home = Path::new("/home/user/.aether");
        let args = build_docker_args(cwd, aether_home, &[], &["aether".to_string()]);

        // Only the binary name — nothing after image
        assert_eq!(args.last().unwrap(), SANDBOX_IMAGE);
    }

    #[test]
    fn sandbox_error_display_messages() {
        assert_eq!(
            SandboxError::DockerNotFound.to_string(),
            "Docker is not installed or not in PATH"
        );

        assert!(SandboxError::DockerNotRunning("connection refused".into())
            .to_string()
            .contains("connection refused"));

        let img_err = SandboxError::ImageNotFound("aether-sandbox:latest".into());
        assert!(img_err.to_string().contains("aether-sandbox:latest"));
        assert!(img_err.to_string().contains("cargo build"));

        assert!(SandboxError::HomeNotResolvable
            .to_string()
            .contains("home directory"));

        let io_err = io::Error::new(io::ErrorKind::NotFound, "not found");
        assert!(SandboxError::ExecFailed(io_err)
            .to_string()
            .contains("not found"));
    }
}
