use std::fmt;
use std::path::{Path, PathBuf};

#[allow(dead_code)]
pub struct GitDiffDocument {
    pub repo_root: PathBuf,
    pub files: Vec<FileDiff>,
}

pub struct FileDiff {
    pub old_path: Option<String>,
    pub path: String,
    pub status: FileStatus,
    pub additions: usize,
    pub deletions: usize,
    pub hunks: Vec<Hunk>,
    pub binary: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileStatus {
    Modified,
    Added,
    Deleted,
    Renamed,
}

#[allow(dead_code)]
pub struct Hunk {
    pub header: String,
    pub old_start: usize,
    pub old_count: usize,
    pub new_start: usize,
    pub new_count: usize,
    pub lines: Vec<PatchLine>,
}

pub struct PatchLine {
    pub kind: PatchLineKind,
    pub text: String,
    pub old_line_no: Option<usize>,
    pub new_line_no: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatchLineKind {
    HunkHeader,
    Context,
    Added,
    Removed,
    Meta,
}

#[derive(Debug)]
pub enum GitDiffError {
    NotARepository,
    CommandFailed { stderr: String },
    ParseError(String),
}

impl fmt::Display for GitDiffError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotARepository => write!(f, "Not a git repository"),
            Self::CommandFailed { stderr } => write!(f, "Git command failed: {stderr}"),
            Self::ParseError(msg) => write!(f, "Failed to parse diff: {msg}"),
        }
    }
}

impl std::error::Error for GitDiffError {}

impl FileStatus {
    pub fn marker(self) -> char {
        match self {
            Self::Modified => 'M',
            Self::Added => 'A',
            Self::Deleted => 'D',
            Self::Renamed => 'R',
        }
    }
}

pub async fn load_git_diff(
    working_dir: &Path,
    cached_repo_root: Option<&Path>,
) -> Result<GitDiffDocument, GitDiffError> {
    let repo_root = match cached_repo_root {
        Some(root) => root.to_path_buf(),
        None => resolve_repo_root(working_dir).await?,
    };
    let diff_output = run_git_diff(&repo_root).await?;

    if diff_output.trim().is_empty() {
        return Ok(GitDiffDocument {
            repo_root,
            files: Vec::new(),
        });
    }

    let files = parse_unified_diff(&diff_output)?;
    Ok(GitDiffDocument { repo_root, files })
}

async fn resolve_repo_root(working_dir: &Path) -> Result<PathBuf, GitDiffError> {
    let output = tokio::process::Command::new("git")
        .arg("rev-parse")
        .arg("--show-toplevel")
        .current_dir(working_dir)
        .output()
        .await
        .map_err(|e| GitDiffError::CommandFailed {
            stderr: e.to_string(),
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("not a git repository") {
            return Err(GitDiffError::NotARepository);
        }
        return Err(GitDiffError::CommandFailed {
            stderr: stderr.into_owned(),
        });
    }

    let root = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok(PathBuf::from(root))
}

async fn run_git_diff(repo_root: &Path) -> Result<String, GitDiffError> {
    let output = tokio::process::Command::new("git")
        .args(["diff", "--no-ext-diff", "--find-renames", "--unified=3", "HEAD"])
        .current_dir(repo_root)
        .output()
        .await
        .map_err(|e| GitDiffError::CommandFailed {
            stderr: e.to_string(),
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(GitDiffError::CommandFailed {
            stderr: stderr.into_owned(),
        });
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

pub fn parse_unified_diff(input: &str) -> Result<Vec<FileDiff>, GitDiffError> {
    let mut files = Vec::new();
    let file_chunks = split_diff_files(input);

    for chunk in file_chunks {
        match parse_file_diff(chunk) {
            Ok(file_diff) => files.push(file_diff),
            Err(e) => return Err(e),
        }
    }

    Ok(files)
}

fn split_diff_files(input: &str) -> Vec<&str> {
    let mut chunks = Vec::new();
    let mut start = None;

    for (i, _) in input.match_indices("diff --git ") {
        if let Some(s) = start {
            chunks.push(&input[s..i]);
        }
        start = Some(i);
    }

    if let Some(s) = start {
        chunks.push(&input[s..]);
    }

    chunks
}

fn parse_file_diff(chunk: &str) -> Result<FileDiff, GitDiffError> {
    let lines: Vec<&str> = chunk.lines().collect();
    if lines.is_empty() {
        return Err(GitDiffError::ParseError("Empty diff chunk".to_string()));
    }

    let (old_path, new_path) = parse_diff_header(lines[0])?;
    let mut status = FileStatus::Modified;
    let mut binary = false;
    let mut rename_from: Option<String> = None;
    let mut hunks = Vec::new();
    let mut i = 1;

    while i < lines.len() {
        let line = lines[i];
        if line.starts_with("new file mode") {
            status = FileStatus::Added;
        } else if line.starts_with("deleted file mode") {
            status = FileStatus::Deleted;
        } else if let Some(from) = line.strip_prefix("rename from ") {
            status = FileStatus::Renamed;
            rename_from = Some(from.to_string());
        } else if line.starts_with("rename to ") {
            status = FileStatus::Renamed;
        } else if line.starts_with("Binary files ") {
            binary = true;
        } else if line.starts_with("@@") {
            break;
        }
        i += 1;
    }

    if !binary {
        while i < lines.len() {
            if lines[i].starts_with("@@") {
                let (hunk, consumed) = parse_hunk(&lines[i..])?;
                hunks.push(hunk);
                i += consumed;
            } else {
                i += 1;
            }
        }
    }

    let mut additions = 0;
    let mut deletions = 0;
    for hunk in &hunks {
        for patch_line in &hunk.lines {
            match patch_line.kind {
                PatchLineKind::Added => additions += 1,
                PatchLineKind::Removed => deletions += 1,
                _ => {}
            }
        }
    }

    let old_path_value = if status == FileStatus::Renamed {
        rename_from.or(Some(old_path))
    } else if status == FileStatus::Added {
        None
    } else {
        Some(old_path)
    };

    Ok(FileDiff {
        old_path: old_path_value,
        path: new_path,
        status,
        additions,
        deletions,
        hunks,
        binary,
    })
}

fn parse_diff_header(line: &str) -> Result<(String, String), GitDiffError> {
    let rest = line
        .strip_prefix("diff --git ")
        .ok_or_else(|| GitDiffError::ParseError(format!("Invalid diff header: {line}")))?;

    if let Some((a, b)) = rest.split_once(" b/") {
        let old = a.strip_prefix("a/").unwrap_or(a).to_string();
        let new = b.to_string();
        Ok((old, new))
    } else {
        Err(GitDiffError::ParseError(format!(
            "Cannot parse paths from: {line}"
        )))
    }
}

fn parse_hunk(lines: &[&str]) -> Result<(Hunk, usize), GitDiffError> {
    let header = lines[0];
    let (old_start, old_count, new_start, new_count) = parse_hunk_header(header)?;

    let mut patch_lines = Vec::new();
    patch_lines.push(PatchLine {
        kind: PatchLineKind::HunkHeader,
        text: header.to_string(),
        old_line_no: None,
        new_line_no: None,
    });

    let mut old_line = old_start;
    let mut new_line = new_start;
    let mut i = 1;

    while i < lines.len() {
        let line = lines[i];
        if line.starts_with("@@") {
            break;
        }

        if let Some(text) = line.strip_prefix('+') {
            patch_lines.push(PatchLine {
                kind: PatchLineKind::Added,
                text: text.to_string(),
                old_line_no: None,
                new_line_no: Some(new_line),
            });
            new_line += 1;
        } else if let Some(text) = line.strip_prefix('-') {
            patch_lines.push(PatchLine {
                kind: PatchLineKind::Removed,
                text: text.to_string(),
                old_line_no: Some(old_line),
                new_line_no: None,
            });
            old_line += 1;
        } else if let Some(text) = line.strip_prefix(' ') {
            patch_lines.push(PatchLine {
                kind: PatchLineKind::Context,
                text: text.to_string(),
                old_line_no: Some(old_line),
                new_line_no: Some(new_line),
            });
            old_line += 1;
            new_line += 1;
        } else if line.starts_with('\\') {
            patch_lines.push(PatchLine {
                kind: PatchLineKind::Meta,
                text: line.to_string(),
                old_line_no: None,
                new_line_no: None,
            });
        } else {
            // Treat as context (git sometimes omits the leading space for empty lines)
            patch_lines.push(PatchLine {
                kind: PatchLineKind::Context,
                text: line.to_string(),
                old_line_no: Some(old_line),
                new_line_no: Some(new_line),
            });
            old_line += 1;
            new_line += 1;
        }
        i += 1;
    }

    Ok((
        Hunk {
            header: header.to_string(),
            old_start,
            old_count,
            new_start,
            new_count,
            lines: patch_lines,
        },
        i,
    ))
}

fn parse_hunk_header(header: &str) -> Result<(usize, usize, usize, usize), GitDiffError> {
    // Format: @@ -old_start,old_count +new_start,new_count @@
    let err = || GitDiffError::ParseError(format!("Invalid hunk header: {header}"));

    let rest = header.strip_prefix("@@ -").ok_or_else(err)?;
    let at_end = rest.find(" @@").ok_or_else(err)?;
    let range_part = &rest[..at_end];

    let (old_range, new_range) = range_part.split_once(" +").ok_or_else(err)?;

    let (old_start, old_count) = parse_range(old_range).ok_or_else(err)?;
    let (new_start, new_count) = parse_range(new_range).ok_or_else(err)?;

    Ok((old_start, old_count, new_start, new_count))
}

fn parse_range(s: &str) -> Option<(usize, usize)> {
    if let Some((start, count)) = s.split_once(',') {
        Some((start.parse().ok()?, count.parse().ok()?))
    } else {
        let start: usize = s.parse().ok()?;
        Some((start, 1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_modified_file() {
        let input = "\
diff --git a/src/main.rs b/src/main.rs
index abc1234..def5678 100644
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,3 +1,4 @@
 fn main() {
+    println!(\"hello\");
     let x = 1;
 }
";
        let files = parse_unified_diff(input).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "src/main.rs");
        assert_eq!(files[0].status, FileStatus::Modified);
        assert_eq!(files[0].additions, 1);
        assert_eq!(files[0].deletions, 0);
        assert!(!files[0].binary);
        assert_eq!(files[0].hunks.len(), 1);
    }

    #[test]
    fn parse_added_file() {
        let input = "\
diff --git a/new_file.txt b/new_file.txt
new file mode 100644
index 0000000..abc1234
--- /dev/null
+++ b/new_file.txt
@@ -0,0 +1,2 @@
+line one
+line two
";
        let files = parse_unified_diff(input).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "new_file.txt");
        assert_eq!(files[0].status, FileStatus::Added);
        assert!(files[0].old_path.is_none());
        assert_eq!(files[0].additions, 2);
        assert_eq!(files[0].deletions, 0);
    }

    #[test]
    fn parse_deleted_file() {
        let input = "\
diff --git a/old_file.txt b/old_file.txt
deleted file mode 100644
index abc1234..0000000
--- a/old_file.txt
+++ /dev/null
@@ -1,2 +0,0 @@
-line one
-line two
";
        let files = parse_unified_diff(input).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "old_file.txt");
        assert_eq!(files[0].status, FileStatus::Deleted);
        assert_eq!(files[0].additions, 0);
        assert_eq!(files[0].deletions, 2);
    }

    #[test]
    fn parse_renamed_file() {
        let input = "\
diff --git a/old_name.rs b/new_name.rs
similarity index 95%
rename from old_name.rs
rename to new_name.rs
index abc1234..def5678 100644
--- a/old_name.rs
+++ b/new_name.rs
@@ -1,3 +1,3 @@
 fn main() {
-    old();
+    new();
 }
";
        let files = parse_unified_diff(input).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "new_name.rs");
        assert_eq!(files[0].status, FileStatus::Renamed);
        assert_eq!(files[0].old_path.as_deref(), Some("old_name.rs"));
        assert_eq!(files[0].additions, 1);
        assert_eq!(files[0].deletions, 1);
    }

    #[test]
    fn parse_hunk_header_tracking() {
        let input = "\
diff --git a/file.rs b/file.rs
index abc..def 100644
--- a/file.rs
+++ b/file.rs
@@ -10,4 +10,5 @@ fn context_label() {
 context
-removed
+added1
+added2
 context
";
        let files = parse_unified_diff(input).unwrap();
        let hunk = &files[0].hunks[0];
        assert_eq!(hunk.old_start, 10);
        assert_eq!(hunk.old_count, 4);
        assert_eq!(hunk.new_start, 10);
        assert_eq!(hunk.new_count, 5);

        // Check line number tracking
        let lines = &hunk.lines;
        // HunkHeader
        assert_eq!(lines[0].kind, PatchLineKind::HunkHeader);
        // context at old=10, new=10
        assert_eq!(lines[1].kind, PatchLineKind::Context);
        assert_eq!(lines[1].old_line_no, Some(10));
        assert_eq!(lines[1].new_line_no, Some(10));
        // removed at old=11
        assert_eq!(lines[2].kind, PatchLineKind::Removed);
        assert_eq!(lines[2].old_line_no, Some(11));
        assert_eq!(lines[2].new_line_no, None);
        // added at new=11
        assert_eq!(lines[3].kind, PatchLineKind::Added);
        assert_eq!(lines[3].old_line_no, None);
        assert_eq!(lines[3].new_line_no, Some(11));
        // added at new=12
        assert_eq!(lines[4].kind, PatchLineKind::Added);
        assert_eq!(lines[4].old_line_no, None);
        assert_eq!(lines[4].new_line_no, Some(12));
        // context at old=12, new=13
        assert_eq!(lines[5].kind, PatchLineKind::Context);
        assert_eq!(lines[5].old_line_no, Some(12));
        assert_eq!(lines[5].new_line_no, Some(13));
    }

    #[test]
    fn parse_meta_line() {
        let input = "\
diff --git a/file.txt b/file.txt
index abc..def 100644
--- a/file.txt
+++ b/file.txt
@@ -1,1 +1,1 @@
-old
\\ No newline at end of file
+new
";
        let files = parse_unified_diff(input).unwrap();
        let hunk = &files[0].hunks[0];
        let meta = hunk.lines.iter().find(|l| l.kind == PatchLineKind::Meta);
        assert!(meta.is_some());
        assert!(meta.unwrap().text.contains("No newline"));
    }

    #[test]
    fn parse_binary_diff() {
        let input = "\
diff --git a/image.png b/image.png
new file mode 100644
index 0000000..abc1234
Binary files /dev/null and b/image.png differ
";
        let files = parse_unified_diff(input).unwrap();
        assert_eq!(files.len(), 1);
        assert!(files[0].binary);
        assert!(files[0].hunks.is_empty());
    }

    #[test]
    fn parse_empty_diff() {
        let files = parse_unified_diff("").unwrap();
        assert!(files.is_empty());
    }

    #[test]
    fn parse_multiple_files() {
        let input = "\
diff --git a/a.rs b/a.rs
index abc..def 100644
--- a/a.rs
+++ b/a.rs
@@ -1,1 +1,1 @@
-old_a
+new_a
diff --git a/b.rs b/b.rs
new file mode 100644
index 0000000..abc1234
--- /dev/null
+++ b/b.rs
@@ -0,0 +1,1 @@
+new_b
";
        let files = parse_unified_diff(input).unwrap();
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].path, "a.rs");
        assert_eq!(files[0].status, FileStatus::Modified);
        assert_eq!(files[1].path, "b.rs");
        assert_eq!(files[1].status, FileStatus::Added);
    }

    #[test]
    fn parse_multiple_hunks() {
        let input = "\
diff --git a/file.rs b/file.rs
index abc..def 100644
--- a/file.rs
+++ b/file.rs
@@ -1,3 +1,3 @@
 fn a() {
-    old_a();
+    new_a();
 }
@@ -10,3 +10,3 @@
 fn b() {
-    old_b();
+    new_b();
 }
";
        let files = parse_unified_diff(input).unwrap();
        assert_eq!(files[0].hunks.len(), 2);
        assert_eq!(files[0].hunks[0].old_start, 1);
        assert_eq!(files[0].hunks[1].old_start, 10);
    }

    #[test]
    fn parse_hunk_header_without_comma() {
        let (start, count, new_start, new_count) =
            parse_hunk_header("@@ -1 +1 @@ fn main()").unwrap();
        assert_eq!(start, 1);
        assert_eq!(count, 1);
        assert_eq!(new_start, 1);
        assert_eq!(new_count, 1);
    }

    #[test]
    fn file_status_marker() {
        assert_eq!(FileStatus::Modified.marker(), 'M');
        assert_eq!(FileStatus::Added.marker(), 'A');
        assert_eq!(FileStatus::Deleted.marker(), 'D');
        assert_eq!(FileStatus::Renamed.marker(), 'R');
    }
}
