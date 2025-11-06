use aether::agent::Prompt;
use crucible::{Eval, EvalAssertion, WorkingDirectory};
use std::path::PathBuf;

/// Returns all mcp-lexicon evals defined programmatically
pub fn all_evals() -> Result<Vec<Eval>, Box<dyn std::error::Error>> {
    let tests_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests");

    Ok(vec![
        Eval::new(
            "simple_bash_command",
            load_prompt("simple_bash_command")?,
            WorkingDirectory::empty()?,
            vec![EvalAssertion::llm_judge(
                "Did the agent successfully run the echo command and display the output 'Hello from bash!'?",
            )],
        ),
        Eval::new(
            "create_new_file",
            load_prompt("create_new_file")?,
            WorkingDirectory::local(tests_dir.join("evals/create_new_file/src"))?,
            vec![
                EvalAssertion::file_exists("src/config.rs"),
                EvalAssertion::file_matches("src/config.rs", "struct Config"),
                EvalAssertion::file_matches("src/config.rs", "name"),
            ],
        ),
        Eval::new(
            "edit_single_file",
            load_prompt("edit_single_file")?,
            WorkingDirectory::local(tests_dir.join("evals/edit_single_file/src"))?,
            vec![EvalAssertion::file_matches("src/main.rs", "Hello, World!")],
        ),
        Eval::new(
            "command_chaining",
            load_prompt("command_chaining")?,
            WorkingDirectory::empty()?,
            vec![
                EvalAssertion::file_exists("build"),
                EvalAssertion::file_exists("build/output.txt"),
                EvalAssertion::file_matches("build/output.txt", "Build successful"),
                EvalAssertion::llm_judge(
                    "Did the agent successfully use chained commands with && to create the build directory, create output.txt with 'Build successful', and list the directory contents?",
                ),
            ],
        ),
        Eval::new(
            "environment_check",
            load_prompt("environment_check")?,
            WorkingDirectory::empty()?,
            vec![EvalAssertion::llm_judge(
                "Did the agent successfully run 'pwd' to show the current directory and 'echo $PATH' to display the PATH environment variable?",
            )],
        ),
        Eval::new(
            "git_operations",
            load_prompt("git_operations")?,
            WorkingDirectory::empty()?,
            vec![
                EvalAssertion::file_exists(".git"),
                EvalAssertion::file_exists("README.md"),
                EvalAssertion::file_matches("README.md", "# My Project"),
                EvalAssertion::llm_judge(
                    "Did the agent successfully initialize a git repository, create README.md, add it, commit it with message 'Initial commit', and show the git status?",
                ),
            ],
        ),
        Eval::new(
            "list_directory",
            load_prompt("list_directory")?,
            WorkingDirectory::local(tests_dir.join("evals/list_directory/src"))?,
            vec![EvalAssertion::llm_judge(
                "Did the agent successfully run 'ls -la' on the src directory and display the directory contents including main.rs and helper.rs?",
            )],
        ),
        Eval::new(
            "search_find_file",
            load_prompt("search_find_file")?,
            WorkingDirectory::local(tests_dir.join("evals/search_find_file/src"))?,
            vec![EvalAssertion::llm_judge(
                "Did the agent successfully identify which files contain 'TODO'? The correct files are: lib.rs and main.rs",
            )],
        ),
        Eval::new(
            "rust_combinations",
            load_prompt("rust_combinations")?,
            WorkingDirectory::local(tests_dir.join("evals/rust_combinations/src"))?,
            vec![
                EvalAssertion::llm_judge(
                    "Did the agent successfully write a Rust program that computes all combinations of a set of characters? The program should generate combinations of different lengths (1-char, 2-char, etc.) and compile successfully.",
                ),
                EvalAssertion::file_matches("src/main.rs", "fn main"),
                EvalAssertion::file_matches("src/main.rs", "combinations"),
                EvalAssertion::llm_judge(
                    "Does the code implement logic to generate combinations (not permutations)? Look for iteration or recursion that generates subsets of different sizes.",
                ),
                EvalAssertion::command_exit_code("cargo check", 0),
                EvalAssertion::command_exit_code("cargo build", 0),
            ],
        ),
        Eval::new(
            "todo_write_complex",
            load_prompt("todo_write_complex")?,
            WorkingDirectory::empty()?,
            vec![
                EvalAssertion::tool_call_exact("coding__todo_write", 1),
                EvalAssertion::tool_call_with_args(
                    "coding__todo_write",
                    serde_json::json!({
                        "todos": [
                            {
                                "content": "Set up database schema",
                                "active_form": "Setting up database schema",
                                "state": "pending"
                            },
                            {
                                "content": "Create user authentication",
                                "active_form": "Creating user authentication",
                                "state": "pending"
                            },
                            {
                                "content": "Design frontend components",
                                "active_form": "Designing frontend components",
                                "state": "pending"
                            },
                            {
                                "content": "Implement user authentication",
                                "active_form": "Implementing user authentication",
                                "state": "in_progress"
                            },
                            {
                                "content": "Build REST API endpoints",
                                "active_form": "Building REST API endpoints",
                                "state": "in_progress"
                            },
                            {
                                "content": "Write unit tests",
                                "active_form": "Writing unit tests",
                                "state": "completed"
                            },
                            {
                                "content": "Deploy to staging",
                                "active_form": "Deploying to staging",
                                "state": "completed"
                            }
                        ]
                    }),
                ),
            ],
        ),
    ])
}

fn load_prompt(eval_dir: &str) -> Result<String, Box<dyn std::error::Error>> {
    let tests_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests");
    let prompt_path = tests_dir.join("evals").join(eval_dir).join("prompt.md");
    let prompt = Prompt::file(prompt_path.to_str().ok_or("Invalid path")?, false).build()?;
    Ok(prompt)
}
