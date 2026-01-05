//! Test helpers for e2e tests.

use fantoccini::{Client, ClientBuilder, Locator, elements::Element, error::CmdError};
use serde_json::json;
use std::io::Write;
use std::net::{TcpListener, TcpStream};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use crate::pages::{AgentViewPage, NewAgentModalPage, SidebarPage};

pub const BASE_URL: &str = "http://localhost:8080";

/// Test harness that handles setup/teardown and provides access to page objects.
pub struct TestHarness {
    client: Option<Client>,
    driver_process: Option<Child>,
}

impl TestHarness {
    /// Get a reference to the WebDriver client.
    pub fn client(&self) -> &Client {
        self.client.as_ref().expect("client already closed")
    }
}

/// Error type for test harness setup failures.
#[derive(Debug)]
pub enum SetupError {
    Session(fantoccini::error::NewSessionError),
    Navigation(CmdError),
    DriverSpawn(std::io::Error),
    DriverTimeout,
    PortAllocation(std::io::Error),
}

impl std::fmt::Display for SetupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SetupError::Session(e) => write!(f, "Failed to create WebDriver session: {e}"),
            SetupError::Navigation(e) => write!(f, "Failed to navigate to app: {e}"),
            SetupError::DriverSpawn(e) => write!(f, "Failed to spawn chromedriver: {e}"),
            SetupError::DriverTimeout => write!(f, "Timeout waiting for chromedriver to start"),
            SetupError::PortAllocation(e) => write!(f, "Failed to allocate port: {e}"),
        }
    }
}

impl std::error::Error for SetupError {}

impl From<fantoccini::error::NewSessionError> for SetupError {
    fn from(e: fantoccini::error::NewSessionError) -> Self {
        SetupError::Session(e)
    }
}

impl From<CmdError> for SetupError {
    fn from(e: CmdError) -> Self {
        SetupError::Navigation(e)
    }
}

impl TestHarness {
    /// Create a new test harness, spawning a dedicated chromedriver instance and navigating to the app.
    pub async fn new() -> Result<Self, SetupError> {
        let port = find_available_port()?;

        let verbose = std::env::var("E2E_VERBOSE").is_ok();
        let process = Command::new("chromedriver")
            .arg(format!("--port={}", port))
            .stdout(if verbose {
                Stdio::inherit()
            } else {
                Stdio::null()
            })
            .stderr(if verbose {
                Stdio::inherit()
            } else {
                Stdio::null()
            })
            .spawn()
            .map_err(SetupError::DriverSpawn)?;

        if verbose {
            eprintln!("[e2e] Started chromedriver on port {}", port);
            std::io::stderr().flush().ok();
        }

        wait_for_port(port).await?;

        if verbose {
            eprintln!("[e2e] Chromedriver accepting connections");
            std::io::stderr().flush().ok();
        }

        let headless = std::env::var("HEADLESS")
            .map(|v| v != "false")
            .unwrap_or(true);

        let mut caps = serde_json::Map::new();
        let mut chrome_opts = serde_json::Map::new();

        let mut args = vec![
            "--disable-gpu".to_string(),
            "--no-sandbox".to_string(),
            "--disable-dev-shm-usage".to_string(),
            "--window-size=1280,720".to_string(),
        ];

        if headless {
            args.push("--headless=new".to_string());
        }

        chrome_opts.insert("args".into(), json!(args));
        caps.insert("goog:chromeOptions".into(), chrome_opts.into());

        if verbose {
            eprintln!("[e2e] Creating WebDriver session...");
            std::io::stderr().flush().ok();
        }

        let client = ClientBuilder::native()
            .capabilities(caps)
            .connect(&format!("http://localhost:{}", port))
            .await?;

        if verbose {
            eprintln!("[e2e] WebDriver session created, navigating to app...");
            std::io::stderr().flush().ok();
        }

        goto_app(&client).await?;

        if verbose {
            eprintln!("[e2e] Navigation complete, harness ready");
            std::io::stderr().flush().ok();
        }

        Ok(Self {
            client: Some(client),
            driver_process: Some(process),
        })
    }

    /// Get a SidebarPage for interacting with the sidebar.
    pub fn sidebar(&self) -> SidebarPage<'_> {
        SidebarPage::new(self.client())
    }

    /// Get a NewAgentModalPage for interacting with the new agent modal.
    pub fn modal(&self) -> NewAgentModalPage<'_> {
        NewAgentModalPage::new(self.client())
    }

    /// Get an AgentViewPage for interacting with the agent view.
    pub fn agent_view(&self) -> AgentViewPage<'_> {
        AgentViewPage::new(self.client())
    }

    /// Close the WebDriver session. Call this at the end of each test.
    /// Note: chromedriver process is automatically cleaned up via Drop.
    pub async fn close(mut self) -> Result<(), CmdError> {
        let result = if let Some(client) = self.client.take() {
            client.close().await
        } else {
            Ok(())
        };
        if let Some(mut proc) = self.driver_process.take() {
            proc.kill().ok();
            proc.wait().ok();
        }
        result
    }
}

impl Drop for TestHarness {
    fn drop(&mut self) {
        if let Some(mut proc) = self.driver_process.take() {
            proc.kill().ok();
            proc.wait().ok();
        }
    }
}

/// Assert that an element with the given testid is visible.
pub async fn assert_visible(client: &Client, testid: &str) {
    let element = by_testid(client, testid)
        .await
        .unwrap_or_else(|_| panic!("Element [data-testid=\"{testid}\"] not found"));
    assert!(
        element.is_displayed().await.unwrap_or(false),
        "Element [data-testid=\"{testid}\"] should be visible"
    );
}

/// Assert that an element with the given testid is NOT visible (or doesn't exist).
pub async fn assert_not_visible(client: &Client, testid: &str) {
    assert!(
        !is_visible(client, testid).await,
        "Element [data-testid=\"{testid}\"] should not be visible"
    );
}

/// Check if agents exist, returning None if no agents (for skipping tests).
pub async fn require_agents(harness: &TestHarness) -> Option<()> {
    let items = harness.sidebar().get_agent_items().await.ok()?;
    if items.is_empty() {
        println!("Skipping: no agents exist");
        return None;
    }
    Some(())
}

/// Wait for Dioxus to hydrate the page.
pub async fn wait_for_hydration(client: &Client) -> Result<(), CmdError> {
    client.wait().for_element(Locator::Css("body")).await?;
    tokio::time::sleep(Duration::from_millis(500)).await;
    Ok(())
}

/// Find element by data-testid attribute.
pub async fn by_testid(client: &Client, testid: &str) -> Result<Element, CmdError> {
    client
        .find(Locator::Css(&format!("[data-testid=\"{}\"]", testid)))
        .await
}

/// Check if element with testid exists and is displayed.
pub async fn is_visible(client: &Client, testid: &str) -> bool {
    match by_testid(client, testid).await {
        Ok(el) => el.is_displayed().await.unwrap_or(false),
        Err(_) => false,
    }
}

/// Find elements matching a CSS selector prefix for testid.
pub async fn by_testid_prefix(client: &Client, prefix: &str) -> Result<Vec<Element>, CmdError> {
    client
        .find_all(Locator::Css(&format!("[data-testid^=\"{}\"]", prefix)))
        .await
}

/// Get attribute value of an element.
pub async fn get_attr(element: &Element, attr: &str) -> Result<Option<String>, CmdError> {
    element.attr(attr).await
}

/// Check if an element is enabled (not disabled).
pub async fn is_enabled(element: &Element) -> Result<bool, CmdError> {
    let disabled = element.attr("disabled").await?;
    Ok(disabled.is_none())
}

/// Navigate to the app and wait for hydration.
pub async fn goto_app(client: &Client) -> Result<(), CmdError> {
    client.goto(BASE_URL).await?;
    wait_for_hydration(client).await
}

/// Find an available port for chromedriver.
fn find_available_port() -> Result<u16, SetupError> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(SetupError::PortAllocation)?;
    listener
        .local_addr()
        .map(|addr| addr.port())
        .map_err(SetupError::PortAllocation)
}

/// Wait for a port to accept connections.
async fn wait_for_port(port: u16) -> Result<(), SetupError> {
    let start = Instant::now();
    let timeout = Duration::from_secs(10);

    while start.elapsed() < timeout {
        if TcpStream::connect(("127.0.0.1", port)).is_ok() {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    Err(SetupError::DriverTimeout)
}
