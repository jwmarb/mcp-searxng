pub mod pool;

pub use pool::{BrowserPoolHandle, TabInfo};

use tokio::sync::RwLock;

use crate::error::{CliError, Result};

pub struct BrowserManager {
    inner: RwLock<Option<playwright_cdp::Browser>>,
}

impl BrowserManager {
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(None),
        }
    }

    pub async fn launch(&self, executable_path: Option<&str>) -> Result<()> {
        let mut inner = self.inner.write().await;

        let playwright = playwright_cdp::Playwright::launch().await
            .map_err(|e| CliError::Browser(format!("Failed to launch playwright: {e}")))?;

        let mut opts = playwright_cdp::options::LaunchOptions::default();
        if let Some(path) = executable_path {
            opts = opts.executable_path(path);
        }
        let browser = playwright.chromium()
            .launch_with_options(opts)
            .await
            .map_err(|e| CliError::Browser(format!("Failed to launch browser: {e}")))?;

        *inner = Some(browser);
        Ok(())
    }

    pub async fn get_browser(&self) -> Result<playwright_cdp::Browser> {
        let inner = self.inner.read().await;
        inner.as_ref()
            .cloned()
            .ok_or_else(|| CliError::Browser("Browser not launched".to_string()))
    }

    pub async fn shutdown(&self) -> Result<()> {
        let mut inner = self.inner.write().await;
        if let Some(browser) = inner.take() {
            browser.close()
                .await
                .map_err(|e| CliError::Browser(format!("Failed to close browser: {e}")))?;
        }
        Ok(())
    }
}

impl Default for BrowserManager {
    fn default() -> Self {
        Self::new()
    }
}