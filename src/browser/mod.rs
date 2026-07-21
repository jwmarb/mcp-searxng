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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_browser_manager_new() {
        let manager = BrowserManager::new();
        assert!(manager.inner.blocking_read().is_none());
    }

    #[test]
    fn test_browser_manager_default() {
        let manager = BrowserManager::default();
        assert!(manager.inner.blocking_read().is_none());
    }

    #[tokio::test]
    async fn test_get_browser_not_launched() {
        let manager = BrowserManager::new();
        let result = manager.get_browser().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_shutdown_without_launch() {
        let manager = BrowserManager::new();
        let result = manager.shutdown().await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_tab_info_creation() {
        let tab = TabInfo {
            index: 0,
            title: "Test".to_string(),
            url: "https://test.com".to_string(),
        };
        assert_eq!(tab.index, 0);
        assert_eq!(tab.title, "Test");
        assert_eq!(tab.url, "https://test.com");
    }
}