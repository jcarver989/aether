use color_eyre::Result;

pub trait Summarizer {
    fn summarize(&self, text: &str) -> impl Future<Output = Result<String>>;
}

#[derive(Clone)]
pub struct TruncateSummarizer {
    max_line_length_in_chars: usize,
    max_lines: usize,
}

impl TruncateSummarizer {
    pub fn new(max_line_length_in_chars: usize, max_lines: usize) -> Self {
        Self {
            max_line_length_in_chars,
            max_lines,
        }
    }

    pub fn default() -> Self {
        Self::new(2000, 2000) // 2000 chars per line, 2000 lines max
    }
}
impl Summarizer for TruncateSummarizer {
    async fn summarize(&self, text: &str) -> Result<String> {
        let lines: Vec<&str> = text.lines().collect();
        let mut result_lines = Vec::new();

        for (i, line) in lines.iter().enumerate() {
            if i >= self.max_lines {
                result_lines.push(format!("... [TRUNCATED: {} more lines]", lines.len() - i));
                break;
            }

            if line.len() > self.max_line_length_in_chars {
                let truncated_chars = line.len() - self.max_line_length_in_chars;
                let truncated_line = format!(
                    "{}... [TRUNCATED: {} more chars]",
                    &line
                        .chars()
                        .take(self.max_line_length_in_chars)
                        .collect::<String>(),
                    truncated_chars
                );
                result_lines.push(truncated_line);
            } else {
                result_lines.push(line.to_string());
            }
        }

        Ok(result_lines.join("\n"))
    }
}
