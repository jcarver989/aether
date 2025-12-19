use aether::llm::anthropic::AnthropicProvider;
use aether::llm::{ChatMessage, Context, LlmResponse, ProviderFactory, StreamingModelProvider};
use aether::types::IsoString;
use futures::StreamExt;
use std::error::Error;
use std::io::{self, Write};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt::init();

    let provider = AnthropicProvider::from_env()?;
    let context = Context::new(
        vec![ChatMessage::User {
            content: "hello world?".to_string(),
            timestamp: IsoString::now(),
        }],
        vec![],
    );

    let stream = provider.stream_response(&context);
    let mut stream = Box::pin(stream);

    print!("Response: ");
    io::stdout().flush()?;

    while let Some(result) = stream.next().await {
        match result? {
            LlmResponse::Text { chunk } => {
                print!("{chunk}");
                io::stdout().flush()?;
            }
            LlmResponse::Done => break,
            _ => {}
        }
    }

    println!();
    Ok(())
}
