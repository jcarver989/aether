//! File search module for @-file mentions.
//!
//! Provides fast, git-aware file enumeration and fuzzy matching using the
//! `ignore` and `nucleo` crates respectively.

use crate::error::AetherDesktopError;
use crate::file_types::FileMatch;
use ignore::WalkBuilder;
use nucleo::{Config, Injector, Nucleo, Utf32String};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;

/// Maximum number of files to enumerate to prevent memory issues in huge repos.
const MAX_FILES: usize = 100_000;

/// Global cache of file searchers keyed by working directory.
///
/// This ensures that multiple agent views with the same cwd share a single
/// file index, avoiding duplicate file tree walks and memory usage.
#[derive(Default)]
pub struct FileSearcherCache {
    searchers: HashMap<PathBuf, Arc<Mutex<FileSearcher>>>,
}

impl FileSearcherCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Get or create a file searcher for the given directory.
    ///
    /// If a searcher already exists for this cwd, returns a clone of the Arc.
    /// Otherwise, creates a new searcher and stores it in the cache.
    pub fn get_or_create(&mut self, cwd: PathBuf) -> Arc<Mutex<FileSearcher>> {
        self.searchers
            .entry(cwd.clone())
            .or_insert_with(|| Arc::new(Mutex::new(FileSearcher::new(cwd))))
            .clone()
    }

    /// Check if a searcher exists for the given directory.
    pub fn contains(&self, cwd: &Path) -> bool {
        self.searchers.contains_key(cwd)
    }
}

/// File searcher with fuzzy matching support.
///
/// Uses `ignore` crate to enumerate files (respects .gitignore) and
/// `nucleo` for high-performance fuzzy matching.
pub struct FileSearcher {
    /// The nucleo matcher instance
    matcher: Nucleo<FileMatch>,
    /// Root directory being searched
    root: PathBuf,
    /// Whether files have been indexed
    indexed: bool,
    /// Number of indexed files (cached from last index_files call)
    file_count: usize,
}

impl FileSearcher {
    /// Create a new file searcher for the given directory.
    ///
    /// Files are not enumerated until `index_files()` is called.
    pub fn new(root: PathBuf) -> Self {
        // Create nucleo with 1 column (the file path for matching)
        let config = Config::DEFAULT.match_paths();
        let matcher = Nucleo::new(config, Arc::new(|| {}), None, 1);

        Self {
            matcher,
            root,
            indexed: false,
            file_count: 0,
        }
    }

    /// Get the injector for adding items to the matcher.
    fn injector(&self) -> Injector<FileMatch> {
        self.matcher.injector()
    }

    /// Index all files in the root directory.
    ///
    /// This respects .gitignore and other ignore files. Should be called
    /// once before searching, and can be called again to refresh the index.
    pub fn index_files(&mut self) -> Result<usize, AetherDesktopError> {
        // Clear previous index
        self.matcher.restart(true);

        let injector = self.injector();
        let mut count = 0;

        // Walk the directory tree, respecting .gitignore
        let walker = WalkBuilder::new(&self.root)
            .hidden(false) // Include hidden files (but .gitignore still applies)
            .git_ignore(true)
            .git_global(true)
            .git_exclude(true)
            .build();

        for entry in walker {
            let entry = entry.map_err(|e| AetherDesktopError::FileSearch(e.to_string()))?;
            let path = entry.path();

            // Skip directories, only index files
            if !path.is_file() {
                continue;
            }

            // Get file metadata for size
            let metadata = path.metadata().ok();
            let size = metadata.map(|m| m.len()).unwrap_or(0);

            // Calculate relative path for display
            let relative_path = path
                .strip_prefix(&self.root)
                .unwrap_or(path)
                .to_string_lossy()
                .to_string();

            let file_match = FileMatch {
                path: relative_path,
                absolute_path: path.to_path_buf(),
                size,
            };

            // Push to matcher with the path as the searchable column
            injector.push(file_match, |m, cols| {
                cols[0] = Utf32String::from(m.path.as_str());
            });

            count += 1;
            if count >= MAX_FILES {
                break;
            }
        }

        // Process the matcher to update the snapshot
        self.matcher.tick(100);

        self.indexed = true;
        self.file_count = count;
        Ok(count)
    }

    /// Search for files matching the query.
    ///
    /// Returns up to `limit` matches sorted by score (best first).
    /// If `index_files()` hasn't been called, this will return an empty vec.
    pub fn search(&mut self, query: &str, limit: usize) -> Vec<FileMatch> {
        if !self.indexed {
            return Vec::new();
        }

        // Update the search pattern
        self.matcher.pattern.reparse(
            0,
            query,
            nucleo::pattern::CaseMatching::Smart,
            nucleo::pattern::Normalization::Smart,
            query.starts_with('/'),
        );

        // Process matches (non-blocking with short timeout)
        self.matcher.tick(10);

        // Collect results from snapshot
        // Results are already sorted by score (best matches first)
        let snapshot = self.matcher.snapshot();
        let match_count = snapshot.matched_item_count();
        let take_count = match_count.min(limit as u32) as usize;

        snapshot
            .matched_items(0..take_count as u32)
            .map(|item| item.data.clone())
            .collect()
    }

    /// Check if files have been indexed.
    pub fn is_indexed(&self) -> bool {
        self.indexed
    }

    /// Get the root directory being searched.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Get the total number of indexed files.
    pub fn file_count(&self) -> usize {
        self.file_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_files(dir: &Path) {
        // Create a simple file structure
        fs::write(dir.join("main.rs"), "fn main() {}").unwrap();
        fs::write(dir.join("lib.rs"), "pub mod foo;").unwrap();

        let src_dir = dir.join("src");
        fs::create_dir_all(&src_dir).unwrap();
        fs::write(src_dir.join("app.rs"), "struct App;").unwrap();
        fs::write(src_dir.join("utils.rs"), "fn helper() {}").unwrap();

        let nested = dir.join("src/components");
        fs::create_dir_all(&nested).unwrap();
        fs::write(nested.join("button.rs"), "struct Button;").unwrap();
        fs::write(nested.join("input.rs"), "struct Input;").unwrap();
    }

    fn create_gitignore(dir: &Path, patterns: &[&str]) {
        let content = patterns.join("\n");
        fs::write(dir.join(".gitignore"), content).unwrap();
    }

    #[test]
    fn test_file_searcher_new() {
        let temp_dir = TempDir::new().unwrap();
        let searcher = FileSearcher::new(temp_dir.path().to_path_buf());

        assert!(!searcher.is_indexed());
        assert_eq!(searcher.root(), temp_dir.path());
    }

    #[test]
    fn test_index_files() {
        let temp_dir = TempDir::new().unwrap();
        create_test_files(temp_dir.path());

        let mut searcher = FileSearcher::new(temp_dir.path().to_path_buf());
        let count = searcher.index_files().unwrap();

        assert!(searcher.is_indexed());
        assert_eq!(count, 6); // 6 .rs files
        assert_eq!(searcher.file_count(), 6);
    }

    #[test]
    fn test_index_respects_gitignore() {
        let temp_dir = TempDir::new().unwrap();
        create_test_files(temp_dir.path());

        // Create a .git directory so ignore crate treats this as a git repo
        fs::create_dir_all(temp_dir.path().join(".git")).unwrap();

        // Create target directory that should be ignored
        let target_dir = temp_dir.path().join("target");
        fs::create_dir_all(&target_dir).unwrap();
        fs::write(target_dir.join("debug.rs"), "ignored").unwrap();
        fs::write(target_dir.join("release.rs"), "also ignored").unwrap();

        // Create .gitignore
        create_gitignore(temp_dir.path(), &["target/"]);

        let mut searcher = FileSearcher::new(temp_dir.path().to_path_buf());
        let count = searcher.index_files().unwrap();

        // Should have 6 .rs files + 1 .gitignore = 7 files (target/* excluded)
        assert_eq!(count, 7);

        // Verify target files are not included in search results
        let results = searcher.search("debug", 10);
        assert!(
            results.is_empty() || !results.iter().any(|r| r.path.contains("target")),
            "target/ files should be excluded"
        );
    }

    #[test]
    fn test_search_empty_query() {
        let temp_dir = TempDir::new().unwrap();
        create_test_files(temp_dir.path());

        let mut searcher = FileSearcher::new(temp_dir.path().to_path_buf());
        searcher.index_files().unwrap();

        // Empty query should return all files (up to limit)
        let results = searcher.search("", 10);
        assert_eq!(results.len(), 6);
    }

    #[test]
    fn test_search_with_query() {
        let temp_dir = TempDir::new().unwrap();
        create_test_files(temp_dir.path());

        let mut searcher = FileSearcher::new(temp_dir.path().to_path_buf());
        searcher.index_files().unwrap();

        // Search for "button"
        let results = searcher.search("button", 10);
        assert!(!results.is_empty());
        assert!(results[0].path.contains("button"));
    }

    #[test]
    fn test_search_fuzzy_matching() {
        let temp_dir = TempDir::new().unwrap();
        create_test_files(temp_dir.path());

        let mut searcher = FileSearcher::new(temp_dir.path().to_path_buf());
        searcher.index_files().unwrap();

        // Fuzzy search "btn" should match "button.rs"
        let results = searcher.search("btn", 10);
        assert!(!results.is_empty());
        // The top result should be button.rs
        assert!(results.iter().any(|r| r.path.contains("button")));
    }

    #[test]
    fn test_search_limit() {
        let temp_dir = TempDir::new().unwrap();
        create_test_files(temp_dir.path());

        let mut searcher = FileSearcher::new(temp_dir.path().to_path_buf());
        searcher.index_files().unwrap();

        // Request only 2 results
        let results = searcher.search("", 2);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_search_not_indexed() {
        let temp_dir = TempDir::new().unwrap();
        let mut searcher = FileSearcher::new(temp_dir.path().to_path_buf());

        // Search without indexing should return empty
        let results = searcher.search("main", 10);
        assert!(results.is_empty());
    }

    #[test]
    fn test_file_match_has_correct_paths() {
        let temp_dir = TempDir::new().unwrap();
        create_test_files(temp_dir.path());

        let mut searcher = FileSearcher::new(temp_dir.path().to_path_buf());
        searcher.index_files().unwrap();

        let results = searcher.search("main", 10);
        assert!(!results.is_empty());

        let main_match = results.iter().find(|r| r.path == "main.rs").unwrap();
        assert_eq!(main_match.path, "main.rs");
        assert_eq!(main_match.absolute_path, temp_dir.path().join("main.rs"));
        // File should have content "fn main() {}" = 12 bytes
        assert!(main_match.size > 0);
    }

    #[test]
    fn test_reindex_clears_previous() {
        let temp_dir = TempDir::new().unwrap();
        create_test_files(temp_dir.path());

        let mut searcher = FileSearcher::new(temp_dir.path().to_path_buf());

        // Index once
        let count1 = searcher.index_files().unwrap();
        assert_eq!(count1, 6);

        // Add a new file
        fs::write(temp_dir.path().join("new_file.rs"), "new").unwrap();

        // Reindex should pick up new file and not duplicate old ones
        let count2 = searcher.index_files().unwrap();
        assert_eq!(count2, 7);
        assert_eq!(searcher.file_count(), 7);
    }

    #[test]
    fn test_cache_returns_same_instance_for_same_cwd() {
        let temp_dir = TempDir::new().unwrap();
        let cwd = temp_dir.path().to_path_buf();

        let mut cache = FileSearcherCache::new();

        // First call should create a new searcher
        let searcher1 = cache.get_or_create(cwd.clone());

        // Second call with same cwd should return the same Arc
        let searcher2 = cache.get_or_create(cwd.clone());

        // Both should point to the same allocation
        assert!(Arc::ptr_eq(&searcher1, &searcher2));
    }

    #[test]
    fn test_cache_returns_different_instances_for_different_cwds() {
        let temp_dir1 = TempDir::new().unwrap();
        let temp_dir2 = TempDir::new().unwrap();

        let mut cache = FileSearcherCache::new();

        let searcher1 = cache.get_or_create(temp_dir1.path().to_path_buf());
        let searcher2 = cache.get_or_create(temp_dir2.path().to_path_buf());

        // Should be different instances
        assert!(!Arc::ptr_eq(&searcher1, &searcher2));
    }

    #[test]
    fn test_cache_contains() {
        let temp_dir = TempDir::new().unwrap();
        let cwd = temp_dir.path().to_path_buf();

        let mut cache = FileSearcherCache::new();

        assert!(!cache.contains(&cwd));

        cache.get_or_create(cwd.clone());

        assert!(cache.contains(&cwd));
    }
}
