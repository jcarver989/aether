/// Trait for file system operations.
/// Allows swapping in fakes for testing
pub trait Fs {
    fn write_file(&self, path: &str, content: &str) -> impl Future<Output = Result<(), String>>;
    fn read_file(&self, path: &str) -> impl Future<Output = Result<String, String>>;
    fn list_files(&self) -> impl Future<Output = Result<Vec<String>, String>>;
    fn file_exists(&self, path: &str) -> impl Future<Output = bool>;
}
