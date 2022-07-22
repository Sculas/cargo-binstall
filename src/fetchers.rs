use std::path::Path;
use std::sync::Arc;

pub use gh_crate_meta::*;
pub use log::debug;
pub use quickinstall::*;
use reqwest::Client;

use crate::{AutoAbortJoinHandle, BinstallError, PkgFmt, PkgMeta};

mod gh_crate_meta;
mod quickinstall;

#[async_trait::async_trait]
pub trait Fetcher: Send + Sync {
    /// Create a new fetcher from some data
    async fn new(client: &Client, data: &Data) -> Arc<Self>
    where
        Self: Sized;

    /// Fetch a package and extract
    async fn fetch_and_extract(&self, dst: &Path) -> Result<(), BinstallError>;

    /// Check if a package is available for download
    async fn check(&self) -> Result<bool, BinstallError>;

    /// Return the package format
    fn pkg_fmt(&self) -> PkgFmt;

    /// A short human-readable name or descriptor for the package source
    fn source_name(&self) -> String;

    /// Should return true if the remote is from a third-party source
    fn is_third_party(&self) -> bool;

    /// Return the target for this fetcher
    fn target(&self) -> &str;
}

/// Data required to fetch a package
#[derive(Clone, Debug)]
pub struct Data {
    pub name: String,
    pub target: String,
    pub version: String,
    pub repo: Option<String>,
    pub meta: PkgMeta,
}

type FetcherJoinHandle = AutoAbortJoinHandle<Result<bool, BinstallError>>;

#[derive(Default)]
pub struct MultiFetcher(Vec<(Arc<dyn Fetcher>, FetcherJoinHandle)>);

impl MultiFetcher {
    pub fn add(&mut self, fetcher: Arc<dyn Fetcher>) {
        self.0.push((
            fetcher.clone(),
            AutoAbortJoinHandle::new(tokio::spawn(async move { fetcher.check().await })),
        ));
    }

    pub async fn first_available(self) -> Option<Arc<dyn Fetcher>> {
        for (fetcher, handle) in self.0 {
            match handle.await {
                Ok(Ok(true)) => return Some(fetcher),
                Ok(Ok(false)) => (),
                Ok(Err(err)) => {
                    debug!(
                        "Error while checking fetcher {}: {}",
                        fetcher.source_name(),
                        err
                    );
                }
                Err(join_err) => {
                    debug!(
                        "Error while joining the task that checks the fetcher {}: {}",
                        fetcher.source_name(),
                        join_err
                    );
                }
            }
        }

        None
    }
}
