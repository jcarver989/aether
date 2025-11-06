/// Generate eval case from GitHub issue
///
/// This binary creates eval cases from GitHub issues by:
/// 1. Fetching issue details using gh CLI
/// 2. Creating directory structure: tests/evals/<repo-name>/<difficulty>/issue-<id>/
/// 3. Writing prompt.md with issue title and contents
/// 4. Finding the PR that closed the issue and extracting commit SHAs
/// 5. Writing pr.md with before/after commit information
///
/// # Usage
///
/// ```bash
/// cargo run --bin gen-eval -- https://github.com/owner/repo/issues/123 easy
/// cargo run --bin gen-eval -- https://github.com/owner/repo/issues/456 medium
/// ```
use anyhow::{Context, Result, anyhow};
use clap::Parser;
use planning_agent::PrInfo;
use regex::Regex;
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

#[derive(Parser)]
#[command(name = "gen-eval")]
#[command(about = "Generate eval case from GitHub issue")]
struct Cli {
    #[arg(help = "GitHub issue URL (e.g., https://github.com/owner/repo/issues/123)")]
    issue_url: String,

    #[arg(help = "Difficulty level: easy, medium, or hard")]
    difficulty: Difficulty,

    #[arg(
        long = "pr",
        help = "Optional PR URL that closed the issue (e.g., https://github.com/owner/repo/pull/456)"
    )]
    pr_url: Option<String>,
}

#[derive(Debug, Clone, clap::ValueEnum)]
enum Difficulty {
    Easy,
    Medium,
    Hard,
}

impl std::fmt::Display for Difficulty {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Difficulty::Easy => write!(f, "easy"),
            Difficulty::Medium => write!(f, "medium"),
            Difficulty::Hard => write!(f, "hard"),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IssueData {
    title: String,
    body: Option<String>,
    closed_by_pull_requests_references: Vec<PrReference>,
}

#[derive(Debug, Deserialize)]
struct PrReference {
    number: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PrData {
    number: u32,
    merged_at: String,
    base_ref_name: String,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let (owner, repo, issue_number) = parse_github_url(&cli.issue_url)?;
    println!(
        "Creating eval from: {}/{} #{}...",
        owner, repo, issue_number
    );

    let issue = fetch_issue(&owner, &repo, issue_number)?;
    let pr_number = match &cli.pr_url {
        Some(pr_url) => {
            let (pr_owner, pr_repo, pr_num) = parse_pr_url(pr_url)?;
            if pr_owner != owner || pr_repo != repo {
                return Err(anyhow!(
                    "PR must be from the same repository as the issue. Issue: {}/{}, PR: {}/{}",
                    owner,
                    repo,
                    pr_owner,
                    pr_repo
                ));
            }

            println!("Using manually specified PR #{}...", pr_num);
            pr_num
        }

        None => {
            let pr_num = issue
                .closed_by_pull_requests_references
                .first()
                .ok_or_else(|| {
                    anyhow!(
                        "Issue #{} was not closed by a PR. Please provide a PR URL with --pr",
                        issue_number
                    )
                })?
                .number;

            println!("Auto-detected PR #{}...", pr_num);
            pr_num
        }
    };

    let pr = fetch_pr(&owner, &repo, pr_number)?;
    let (before_sha, after_sha) = get_commits_around_pr(&owner, &repo, &pr)?;
    let eval_dir = create_eval_directory(&repo, &cli.difficulty, issue_number)?;

    write_prompt_file(&eval_dir, &issue)?;
    write_pr_json(&eval_dir, &before_sha, &after_sha, &pr)?;

    println!(
        "\nSuccessfully generated eval case at: {}",
        eval_dir.display()
    );

    Ok(())
}

fn parse_github_url(url: &str) -> Result<(String, String, u32)> {
    // Expected format: https://github.com/owner/repo/issues/123
    let re = Regex::new(r"^https?://github\.com/([^/]+)/([^/]+)/issues/(\d+)/?$")
        .expect("Invalid regex pattern");

    let caps = re.captures(url).ok_or_else(|| {
        anyhow!(
            "Invalid GitHub issue URL format. Expected: https://github.com/owner/repo/issues/123"
        )
    })?;

    let owner = caps[1].to_string();
    let repo = caps[2].to_string();
    let issue_number = caps[3]
        .parse::<u32>()
        .context("Failed to parse issue number")?;

    Ok((owner, repo, issue_number))
}

fn parse_pr_url(url: &str) -> Result<(String, String, u32)> {
    // Expected format: https://github.com/owner/repo/pull/456
    let re = Regex::new(r"^https?://github\.com/([^/]+)/([^/]+)/pull/(\d+)/?$")
        .expect("Invalid regex pattern");

    let caps = re.captures(url).ok_or_else(|| {
        anyhow!("Invalid GitHub PR URL format. Expected: https://github.com/owner/repo/pull/456")
    })?;

    let owner = caps[1].to_string();
    let repo = caps[2].to_string();
    let pr_number = caps[3]
        .parse::<u32>()
        .context("Failed to parse PR number")?;

    Ok((owner, repo, pr_number))
}

fn fetch_issue(owner: &str, repo: &str, issue_number: u32) -> Result<IssueData> {
    let output = Command::new("gh")
        .args([
            "issue",
            "view",
            &issue_number.to_string(),
            "--repo",
            &format!("{}/{}", owner, repo),
            "--json",
            "title,body,closedByPullRequestsReferences",
        ])
        .output()
        .context("Failed to execute gh command. Is gh CLI installed?")?;

    if !output.status.success() {
        return Err(anyhow!(
            "gh command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let issue: IssueData =
        serde_json::from_slice(&output.stdout).context("Failed to parse issue JSON")?;

    Ok(issue)
}

fn fetch_pr(owner: &str, repo: &str, pr_number: u32) -> Result<PrData> {
    let output = Command::new("gh")
        .args([
            "pr",
            "view",
            &pr_number.to_string(),
            "--repo",
            &format!("{}/{}", owner, repo),
            "--json",
            "number,mergedAt,baseRefName",
        ])
        .output()
        .context("Failed to execute gh command")?;

    if !output.status.success() {
        return Err(anyhow!(
            "gh command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let pr: PrData = serde_json::from_slice(&output.stdout).context("Failed to parse PR JSON")?;

    Ok(pr)
}

fn get_commits_around_pr(owner: &str, repo: &str, pr: &PrData) -> Result<(String, String)> {
    // Get the 2 most recent commits on the base branch at the time the PR was merged
    // These give us the HEAD commit after merge and the commit right before it
    let output = Command::new("gh")
        .args([
            "api",
            &format!(
                "/repos/{}/{}/commits?sha={}&until={}&per_page=2",
                owner, repo, pr.base_ref_name, pr.merged_at
            ),
            "--jq",
            ".[0].sha, .[1].sha",
        ])
        .output()
        .context("Failed to execute gh command")?;

    if !output.status.success() {
        return Err(anyhow!(
            "gh command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let output_str = String::from_utf8(output.stdout).context("Invalid UTF-8 in gh output")?;
    let commits: Vec<&str> = output_str.lines().collect();

    if commits.len() < 2 {
        return Err(anyhow!("Could not find commits around PR merge time"));
    }

    let after_sha = commits[0].trim().to_string();
    let before_sha = commits[1].trim().to_string();

    if after_sha.is_empty() || before_sha.is_empty() {
        return Err(anyhow!("Failed to parse commit SHAs"));
    }

    Ok((before_sha, after_sha))
}

fn create_eval_directory(
    repo: &str,
    difficulty: &Difficulty,
    issue_number: u32,
) -> Result<PathBuf> {
    let base_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let eval_dir = base_dir
        .join("tests")
        .join("evals")
        .join(repo)
        .join(difficulty.to_string())
        .join(format!("issue-{}", issue_number));

    fs::create_dir_all(&eval_dir).context("Failed to create eval directory")?;

    Ok(eval_dir)
}

fn write_prompt_file(eval_dir: &PathBuf, issue: &IssueData) -> Result<()> {
    let prompt_path = eval_dir.join("prompt.md");
    let body = issue.body.as_deref().unwrap_or("");
    let content = format!("# {}\n\n{}\n", issue.title, body);

    fs::write(&prompt_path, content).context("Failed to write prompt.md")?;

    Ok(())
}

fn write_pr_json(eval_dir: &PathBuf, before_sha: &str, after_sha: &str, pr: &PrData) -> Result<()> {
    let pr_info = PrInfo {
        pr_number: pr.number,
        base_branch: pr.base_ref_name.clone(),
        before_commit: before_sha.to_string(),
        after_commit: after_sha.to_string(),
    };

    let pr_path = eval_dir.join("pr.json");
    let json =
        serde_json::to_string_pretty(&pr_info).context("Failed to serialize PR info to JSON")?;

    fs::write(&pr_path, json).context("Failed to write pr.json")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_github_url() {
        let url = "https://github.com/joist-orm/joist-orm/issues/1406";
        let (owner, repo, issue_num) = parse_github_url(url).unwrap();
        assert_eq!(owner, "joist-orm");
        assert_eq!(repo, "joist-orm");
        assert_eq!(issue_num, 1406);
    }

    #[test]
    fn test_parse_github_url_with_trailing_slash() {
        let url = "https://github.com/joist-orm/joist-orm/issues/1406/";
        let (owner, repo, issue_num) = parse_github_url(url).unwrap();
        assert_eq!(owner, "joist-orm");
        assert_eq!(repo, "joist-orm");
        assert_eq!(issue_num, 1406);
    }

    #[test]
    fn test_parse_github_url_http() {
        let url = "http://github.com/owner/repo/issues/42";
        let (owner, repo, issue_num) = parse_github_url(url).unwrap();
        assert_eq!(owner, "owner");
        assert_eq!(repo, "repo");
        assert_eq!(issue_num, 42);
    }

    #[test]
    fn test_parse_invalid_url_no_issue_number() {
        let url = "https://github.com/joist-orm/joist-orm/issues";
        assert!(parse_github_url(url).is_err());
    }

    #[test]
    fn test_parse_invalid_url_incomplete() {
        let url = "https://github.com/joist-orm";
        assert!(parse_github_url(url).is_err());
    }

    #[test]
    fn test_parse_invalid_url_not_github() {
        let url = "https://gitlab.com/owner/repo/issues/123";
        assert!(parse_github_url(url).is_err());
    }

    #[test]
    fn test_parse_invalid_url_pull_request() {
        let url = "https://github.com/owner/repo/pull/123";
        assert!(parse_github_url(url).is_err());
    }

    #[test]
    fn test_difficulty_display() {
        assert_eq!(Difficulty::Easy.to_string(), "easy");
        assert_eq!(Difficulty::Medium.to_string(), "medium");
        assert_eq!(Difficulty::Hard.to_string(), "hard");
    }

    #[test]
    fn test_parse_pr_url() {
        let url = "https://github.com/joist-orm/joist-orm/pull/1581";
        let (owner, repo, pr_num) = parse_pr_url(url).unwrap();
        assert_eq!(owner, "joist-orm");
        assert_eq!(repo, "joist-orm");
        assert_eq!(pr_num, 1581);
    }

    #[test]
    fn test_parse_pr_url_with_trailing_slash() {
        let url = "https://github.com/owner/repo/pull/123/";
        let (owner, repo, pr_num) = parse_pr_url(url).unwrap();
        assert_eq!(owner, "owner");
        assert_eq!(repo, "repo");
        assert_eq!(pr_num, 123);
    }

    #[test]
    fn test_parse_pr_url_http() {
        let url = "http://github.com/owner/repo/pull/42";
        let (owner, repo, pr_num) = parse_pr_url(url).unwrap();
        assert_eq!(owner, "owner");
        assert_eq!(repo, "repo");
        assert_eq!(pr_num, 42);
    }

    #[test]
    fn test_parse_pr_url_invalid() {
        let url = "https://github.com/owner/repo/issues/123";
        assert!(parse_pr_url(url).is_err());
    }
}
