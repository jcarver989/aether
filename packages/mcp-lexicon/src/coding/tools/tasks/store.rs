use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use thiserror::Error;

use super::index::TaskIndex;
use super::types::{Task, TaskId, TaskResult, TaskStatus, TaskUpdate};

/// Errors that can occur during task store operations
#[derive(Debug, Error)]
pub enum TaskStoreError {
    #[error("Task not found: {id}")]
    NotFound { id: String },

    #[error("Task tree not found: {id}")]
    TreeNotFound { id: String },

    #[error("Task already exists: {id}")]
    AlreadyExists { id: String },

    #[error("Parent task not found: {id}")]
    ParentNotFound { id: String },

    #[error("Dependency not found: {id}")]
    DependencyNotFound { id: String },

    #[error("Cannot create subtask of non-root task: {id}")]
    InvalidParent { id: String },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

/// Task store backed by JSONL files
#[derive(Debug)]
pub struct TaskStore {
    root: PathBuf,
    index: TaskIndex,
    initialized: bool,
}

impl TaskStore {
    /// Create a new task store at the given root directory
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            index: TaskIndex::new(),
            initialized: false,
        }
    }

    /// Initialize the store by loading all active tasks.
    /// This is idempotent - subsequent calls are no-ops.
    pub fn init(&mut self) -> Result<(), TaskStoreError> {
        if self.initialized {
            return Ok(());
        }

        let active_dir = self.active_dir();

        if !active_dir.exists() {
            fs::create_dir_all(&active_dir)?;
        }

        self.index.clear();

        // Scan active directory for task files
        for entry in fs::read_dir(&active_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("jsonl") {
                self.load_tree_file(&path)?;
            }
        }

        self.initialized = true;
        Ok(())
    }

    /// Create a new task tree with a root task
    pub fn create_tree(
        &mut self,
        title: &str,
        description: Option<&str>,
    ) -> Result<Task, TaskStoreError> {
        let hash = self.generate_hash();
        let id = TaskId::new_root(&hash);

        if self.index.contains(&id) {
            return Err(TaskStoreError::AlreadyExists {
                id: id.to_string(),
            });
        }

        let mut task = Task::new_root(id, title.to_string());
        task.description = description.map(String::from);

        let file_path = self.active_dir().join(task.id.filename());
        self.write_task_to_file(&task, &file_path)?;
        self.index.insert(task.clone(), file_path);

        Ok(task)
    }

    /// Add a subtask to an existing root task
    pub fn add_subtask(
        &mut self,
        parent_id: &TaskId,
        title: &str,
    ) -> Result<Task, TaskStoreError> {
        // Validate parent exists and is a root task
        let root_id = parent_id.root();

        if !self.index.contains(&root_id) {
            return Err(TaskStoreError::ParentNotFound {
                id: parent_id.to_string(),
            });
        }

        // Generate subtask ID
        let subtask_index = self.index.next_subtask_index(&root_id);
        let subtask_id = TaskId::new_subtask(parent_id, subtask_index);

        let task = Task::new_subtask(subtask_id, parent_id.clone(), title.to_string());

        // Get the tree file path
        let file_path = self
            .index
            .get_tree_path(&root_id)
            .cloned()
            .ok_or_else(|| TaskStoreError::TreeNotFound {
                id: root_id.to_string(),
            })?;

        // Append to the tree file
        self.write_task_to_file(&task, &file_path)?;
        self.index.insert(task.clone(), file_path);

        Ok(task)
    }

    /// Update a task's fields
    pub fn update(&mut self, id: &TaskId, updates: TaskUpdate) -> Result<Task, TaskStoreError> {
        // Validate dependencies if provided
        if let Some(deps) = &updates.deps {
            for dep_id in deps {
                if !self.index.contains(dep_id) {
                    return Err(TaskStoreError::DependencyNotFound {
                        id: dep_id.to_string(),
                    });
                }
            }
        }

        // Clone the task first, then apply updates
        // (we need the old task in the index for proper re-indexing)
        let mut task = self
            .index
            .get(id)
            .cloned()
            .ok_or_else(|| TaskStoreError::NotFound { id: id.to_string() })?;

        updates.apply_to(&mut task);

        // Update the index first (it needs to see the old task to clean up indices)
        self.index.update(task.clone());

        // Rewrite the tree file
        self.rewrite_tree_file(&task.id.root())?;

        Ok(task)
    }

    /// Mark a task as completed with optional structured result
    pub fn complete(
        &mut self,
        id: &TaskId,
        result: Option<TaskResult>,
        result_text: Option<&str>,
    ) -> Result<Task, TaskStoreError> {
        // Clone the task first (we need the old task in the index for proper re-indexing)
        let mut task = self
            .index
            .get(id)
            .cloned()
            .ok_or_else(|| TaskStoreError::NotFound { id: id.to_string() })?;

        task.status = TaskStatus::Completed;
        task.result = result;
        task.result_text = result_text.map(String::from);
        task.touch();

        // Update the index first (it needs to see the old task to clean up indices)
        self.index.update(task.clone());

        // Rewrite the tree file
        self.rewrite_tree_file(&task.id.root())?;

        Ok(task)
    }

    /// Get a task by ID
    pub fn get(&self, id: &TaskId) -> Option<&Task> {
        self.index.get(id)
    }

    /// Get all tasks in a tree
    pub fn get_tree(&self, root_id: &TaskId) -> Option<Vec<&Task>> {
        if !self.index.contains(root_id) {
            return None;
        }
        Some(self.index.get_tree(root_id))
    }

    /// List tasks by assignee
    pub fn list_by_assignee(&self, assignee: &str) -> Vec<&Task> {
        self.index.get_by_assignee(assignee)
    }

    /// List tasks by status
    pub fn list_by_status(&self, status: TaskStatus) -> Vec<&Task> {
        self.index.get_by_status(status)
    }

    /// Get tasks ready to start (pending with all deps completed)
    pub fn get_ready(&self) -> Vec<&Task> {
        self.index.get_ready()
    }

    /// Archive a completed task tree (move to completed directory)
    pub fn archive_tree(&mut self, root_id: &TaskId) -> Result<(), TaskStoreError> {
        let root_id = root_id.root();

        let src_path = self
            .index
            .get_tree_path(&root_id)
            .cloned()
            .ok_or_else(|| TaskStoreError::TreeNotFound {
                id: root_id.to_string(),
            })?;

        let completed_dir = self.completed_dir();
        if !completed_dir.exists() {
            fs::create_dir_all(&completed_dir)?;
        }

        let dst_path = completed_dir.join(root_id.filename());

        fs::rename(&src_path, &dst_path)?;
        self.index.remove_tree(&root_id);

        Ok(())
    }

    /// Get all root task IDs
    pub fn list_trees(&self) -> Vec<&TaskId> {
        self.index.root_ids()
    }

    /// Get the total number of tasks
    pub fn len(&self) -> usize {
        self.index.len()
    }

    /// Check if the store is empty
    pub fn is_empty(&self) -> bool {
        self.index.is_empty()
    }

    // Private helper methods

    fn active_dir(&self) -> PathBuf {
        self.root.join("active")
    }

    fn completed_dir(&self) -> PathBuf {
        self.root.join("completed")
    }

    fn generate_hash(&self) -> String {
        use std::time::{SystemTime, UNIX_EPOCH};

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();

        // Mix in process ID and existing task count to reduce collision risk
        // on rapid task creation within the same process
        let pid = std::process::id() as u128;
        let count = self.index.len() as u128;
        let mixed = now.wrapping_add(pid << 16).wrapping_add(count);
        format!("{:08x}", (mixed as u64) & 0xFFFFFFFF)
    }

    fn load_tree_file(&mut self, path: &PathBuf) -> Result<(), TaskStoreError> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);

        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }

            let task: Task = serde_json::from_str(&line)?;
            self.index.insert(task, path.clone());
        }

        Ok(())
    }

    fn write_task_to_file(&self, task: &Task, path: &PathBuf) -> Result<(), TaskStoreError> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent()
            && !parent.exists()
        {
            fs::create_dir_all(parent)?;
        }

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;

        let json = serde_json::to_string(task)?;
        writeln!(file, "{}", json)?;

        Ok(())
    }

    fn rewrite_tree_file(&self, root_id: &TaskId) -> Result<(), TaskStoreError> {
        let path = self
            .index
            .get_tree_path(root_id)
            .cloned()
            .ok_or_else(|| TaskStoreError::TreeNotFound {
                id: root_id.to_string(),
            })?;

        // Get all tasks in the tree, sorted by ID
        let mut tasks: Vec<_> = self.index.get_tree(root_id);
        tasks.sort_by_key(|t| t.id.as_str());

        // Write all tasks to file
        let mut file = File::create(&path)?;
        for task in tasks {
            let json = serde_json::to_string(task)?;
            writeln!(file, "{}", json)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup() -> (TempDir, TaskStore) {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path().join(".aether-tasks");
        let mut store = TaskStore::new(root);
        store.init().unwrap();
        (temp_dir, store)
    }

    #[test]
    fn test_create_tree() {
        let (_temp, mut store) = setup();

        let task = store
            .create_tree("Research topic X", Some("Detailed description"))
            .unwrap();

        assert!(task.id.is_root());
        assert_eq!(task.title, "Research topic X");
        assert_eq!(task.description, Some("Detailed description".to_string()));
        assert_eq!(task.status, TaskStatus::Pending);

        // Verify file exists
        let file_path = store.active_dir().join(task.id.filename());
        assert!(file_path.exists());
    }

    #[test]
    fn test_add_subtask() {
        let (_temp, mut store) = setup();

        let root = store.create_tree("Root task", None).unwrap();
        let subtask = store.add_subtask(&root.id, "Subtask 1").unwrap();

        assert!(!subtask.id.is_root());
        assert_eq!(subtask.parent, Some(root.id.clone()));
        assert_eq!(subtask.title, "Subtask 1");

        // Verify tree contains both tasks
        let tree = store.get_tree(&root.id).unwrap();
        assert_eq!(tree.len(), 2);
    }

    #[test]
    fn test_update_task() {
        let (_temp, mut store) = setup();

        let task = store.create_tree("Original title", None).unwrap();

        let updated = store
            .update(
                &task.id,
                TaskUpdate {
                    title: Some("Updated title".to_string()),
                    status: Some(TaskStatus::InProgress),
                    assignee: Some("worker-1".to_string()),
                    ..Default::default()
                },
            )
            .unwrap();

        assert_eq!(updated.title, "Updated title");
        assert_eq!(updated.status, TaskStatus::InProgress);
        assert_eq!(updated.assignee, Some("worker-1".to_string()));
    }

    #[test]
    fn test_complete_task_with_text() {
        let (_temp, mut store) = setup();

        let task = store.create_tree("Task to complete", None).unwrap();

        let completed = store
            .complete(&task.id, None, Some("Found the answer!"))
            .unwrap();

        assert_eq!(completed.status, TaskStatus::Completed);
        assert_eq!(completed.result_text, Some("Found the answer!".to_string()));
    }

    #[test]
    fn test_get_ready() {
        let (_temp, mut store) = setup();

        let root = store.create_tree("Root", None).unwrap();
        let subtask1 = store.add_subtask(&root.id, "Subtask 1").unwrap();
        let subtask2 = store.add_subtask(&root.id, "Subtask 2").unwrap();

        // Add dependency: subtask2 depends on subtask1
        store
            .update(
                &subtask2.id,
                TaskUpdate {
                    deps: Some(vec![subtask1.id.clone()]),
                    ..Default::default()
                },
            )
            .unwrap();

        // Initially: root and subtask1 ready, subtask2 blocked
        let ready = store.get_ready();
        assert_eq!(ready.len(), 2);

        // Complete subtask1
        store.complete(&subtask1.id, None, None).unwrap();

        // Now subtask2 should also be ready
        let ready = store.get_ready();
        assert_eq!(ready.len(), 2); // root and subtask2

        let ready_ids: Vec<_> = ready.iter().map(|t| t.id.as_str()).collect();
        assert!(ready_ids.contains(&subtask2.id.as_str()));
    }

    #[test]
    fn test_archive_tree() {
        let (_temp, mut store) = setup();

        let root = store.create_tree("Completed research", None).unwrap();
        store.add_subtask(&root.id, "Subtask").unwrap();

        let active_file = store.active_dir().join(root.id.filename());
        assert!(active_file.exists());

        store.archive_tree(&root.id).unwrap();

        // Active file should be gone
        assert!(!active_file.exists());

        // Completed file should exist
        let completed_file = store.completed_dir().join(root.id.filename());
        assert!(completed_file.exists());

        // Index should be empty
        assert!(store.is_empty());
    }

    #[test]
    fn test_persistence() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path().join(".aether-tasks");

        // Create store and add tasks
        let task_id = {
            let mut store = TaskStore::new(root.clone());
            store.init().unwrap();

            let task = store.create_tree("Persistent task", None).unwrap();
            store.add_subtask(&task.id, "Subtask 1").unwrap();
            store.add_subtask(&task.id, "Subtask 2").unwrap();
            task.id
        };

        // Create new store and verify tasks are loaded
        let mut store = TaskStore::new(root);
        store.init().unwrap();

        assert_eq!(store.len(), 3);

        let tree = store.get_tree(&task_id).unwrap();
        assert_eq!(tree.len(), 3);
    }

    #[test]
    fn test_list_by_assignee() {
        let (_temp, mut store) = setup();

        let root = store.create_tree("Root", None).unwrap();

        store
            .update(
                &root.id,
                TaskUpdate {
                    assignee: Some("orchestrator".to_string()),
                    ..Default::default()
                },
            )
            .unwrap();

        let subtask = store.add_subtask(&root.id, "Worker task").unwrap();
        store
            .update(
                &subtask.id,
                TaskUpdate {
                    assignee: Some("worker-1".to_string()),
                    ..Default::default()
                },
            )
            .unwrap();

        assert_eq!(store.list_by_assignee("orchestrator").len(), 1);
        assert_eq!(store.list_by_assignee("worker-1").len(), 1);
        assert_eq!(store.list_by_assignee("unknown").len(), 0);
    }

    #[test]
    fn test_list_by_status() {
        let (_temp, mut store) = setup();

        let root = store.create_tree("Root", None).unwrap();
        let subtask = store.add_subtask(&root.id, "Subtask").unwrap();

        // Root is pending, subtask is pending
        assert_eq!(store.list_by_status(TaskStatus::Pending).len(), 2);

        // Start working on subtask
        store
            .update(
                &subtask.id,
                TaskUpdate {
                    status: Some(TaskStatus::InProgress),
                    ..Default::default()
                },
            )
            .unwrap();

        assert_eq!(store.list_by_status(TaskStatus::Pending).len(), 1);
        assert_eq!(store.list_by_status(TaskStatus::InProgress).len(), 1);
    }

    #[test]
    fn test_error_task_not_found() {
        let (_temp, store) = setup();

        let result = store.get(&TaskId::from("at-nonexistent"));
        assert!(result.is_none());
    }

    #[test]
    fn test_error_parent_not_found() {
        let (_temp, mut store) = setup();

        let result = store.add_subtask(&TaskId::from("at-nonexistent"), "Orphan");
        assert!(matches!(result, Err(TaskStoreError::ParentNotFound { .. })));
    }

    #[test]
    fn test_error_dependency_not_found() {
        let (_temp, mut store) = setup();

        let task = store.create_tree("Task", None).unwrap();

        let result = store.update(
            &task.id,
            TaskUpdate {
                deps: Some(vec![TaskId::from("at-nonexistent")]),
                ..Default::default()
            },
        );

        assert!(matches!(
            result,
            Err(TaskStoreError::DependencyNotFound { .. })
        ));
    }

    #[test]
    fn test_init_is_idempotent() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path().join(".aether-tasks");

        let mut store = TaskStore::new(root);
        store.init().unwrap();

        // Create a task
        let task = store.create_tree("Test task", None).unwrap();
        assert_eq!(store.len(), 1);

        // Calling init() again should NOT clear the in-memory index
        store.init().unwrap();
        assert_eq!(store.len(), 1);

        // Task should still be accessible
        let retrieved = store.get(&task.id);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().title, "Test task");
    }
}
