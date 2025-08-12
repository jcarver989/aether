use aether_core::tools::{Summarizer, TruncateSummarizer};

#[tokio::test]
async fn test_truncate_summarizer_small_content() {
    let summarizer = TruncateSummarizer::new(100, 10);
    let small_content = "This is a small piece of content.";

    let result = summarizer.summarize(small_content).await.unwrap();

    assert_eq!(result, small_content);
}

#[tokio::test]
async fn test_truncate_summarizer_long_lines() {
    let summarizer = TruncateSummarizer::new(50, 10);
    let long_line = "x".repeat(200);

    let result = summarizer.summarize(&long_line).await.unwrap();

    assert!(result.contains("TRUNCATED"));
    assert!(result.len() < long_line.len());
    assert!(result.contains("150 more chars"));
}

#[tokio::test]
async fn test_truncate_summarizer_many_lines() {
    let summarizer = TruncateSummarizer::new(100, 5);
    let many_lines = (0..20)
        .map(|i| format!("Line {}", i))
        .collect::<Vec<_>>()
        .join("\n");

    let result = summarizer.summarize(&many_lines).await.unwrap();

    assert!(result.contains("TRUNCATED"));
    assert!(result.contains("15 more lines"));
    let result_lines: Vec<&str> = result.lines().collect();
    assert!(result_lines.len() <= 6); // 5 lines + truncation message
}

#[tokio::test]
async fn test_truncate_summarizer_both_lines_and_chars() {
    let summarizer = TruncateSummarizer::new(30, 3);
    let content = vec![
        "x".repeat(100),            // Long line
        "Short line".to_string(),   // Normal line
        "y".repeat(80),             // Another long line
        "Another line".to_string(), // This should be truncated due to line limit
    ]
    .join("\n");

    let result = summarizer.summarize(&content).await.unwrap();

    assert!(result.contains("TRUNCATED"));
    let result_lines: Vec<&str> = result.lines().collect();
    assert!(result_lines.len() <= 4); // 3 lines + truncation message
}

#[tokio::test]
async fn test_default_summarizer_large_content() {
    let summarizer = TruncateSummarizer::default();

    // Create content larger than the default limits (2000 chars per line, 2000 lines)
    let large_content = (0..3000)
        .map(|i| format!("Line {} with some content", i))
        .collect::<Vec<_>>()
        .join("\n");

    let result = summarizer.summarize(&large_content).await.unwrap();

    assert!(result.contains("TRUNCATED"));
    assert!(result.contains("1000 more lines"));
    let result_lines: Vec<&str> = result.lines().collect();
    assert!(result_lines.len() <= 2001); // 2000 lines + truncation message
}
