use crate::error::{Error, Result};
use crate::IntoSubdomain;
use serde::Deserialize;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Deserialize)]
struct ThreatCrowdResult {
    subdomains: Option<Vec<String>>,
}

impl IntoSubdomain for ThreatCrowdResult {
    fn subdomains(&self) -> HashSet<String> {
        self.subdomains
            .iter()
            .flatten()
            .map(|s| s.to_string())
            .collect()
    }
}

fn build_url(host: &str) -> String {
    format!(
        "https://www.threatcrowd.org/searchApi/v2/domain/report/?domain={}",
        host
    )
}

pub async fn run(host: Arc<String>) -> Result<HashSet<String>> {
    trace!("fetching data from threatcrowd for: {}", &host);
    let uri = build_url(&host);

    let client = reqwest::ClientBuilder::new()
        .timeout(Duration::from_secs(10))
        .pool_idle_timeout(Duration::from_secs(4))
        .build()?;

    let resp: ThreatCrowdResult = client.get(&uri).send().await?.json().await?;
    let subdomains = resp.subdomains();

    debug!("threatcrowd response: {:?}", &resp);
    if !subdomains.is_empty() {
        Ok(subdomains)
    } else {
        Err(Error::source_error("ThreatCrowd", host))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn returns_results() {
        let host = Arc::new("hackerone.com".to_owned());
        let results = run(host).await.unwrap();
        assert!(!results.is_empty());
    }

    #[tokio::test]
    async fn handle_no_results() {
        let host = Arc::new("anVubmxpa2VzdGVh.com".to_string());
        let res = run(host).await;
        let e = res.unwrap_err();
        assert_eq!(
            e.to_string(),
            "ThreatCrowd couldn't find any results for: anVubmxpa2VzdGVh.com"
        );
    }
}
