use crate::error::{Error, Result};
use crate::IntoSubdomain;
use dotenv::dotenv;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::env;
use std::sync::Arc;
use std::time::Duration;

struct Creds {
    url: String,
    api_key: String,
}

impl Creds {
    fn read_creds() -> Result<Self> {
        dotenv().ok();
        let api_key = env::var("INTELX_KEY");
        let url = env::var("INTELX_URL");

        if api_key.is_ok() && url.is_ok() {
            Ok(Self {
                url: url?,
                api_key: api_key?,
            })
        } else {
            Err(Error::key_error("Intelx", &["INTELX_URL", "INTELX_KEY"]))
        }
    }
}

#[derive(Deserialize, Debug)]
struct SearchId {
    id: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct Query {
    term: String,
    maxresults: i32,
    media: usize,
    target: usize,
    timeout: i32,
}

impl Query {
    fn new(term: String) -> Self {
        Self {
            term,
            maxresults: 100000,
            media: 0,
            target: 1,
            timeout: 20,
        }
    }
}

#[derive(Deserialize, Debug)]
struct IntelxItem {
    selectorvalue: String,
}

#[derive(Deserialize, Debug)]
struct IntelxResults {
    selectors: Vec<IntelxItem>,
    status: usize,
}

impl IntoSubdomain for IntelxResults {
    fn subdomains(&self) -> HashSet<String> {
        self.selectors
            .iter()
            .map(|s| s.selectorvalue.to_owned())
            .collect()
    }
}

// 9df61df0-84f7-4dc7-b34c-8ccfb8646ace
fn build_url(intelx_url: &str, api_key: &str, querying: bool, search_id: Option<&str>) -> String {
    if querying {
        format!("https://{}/phonebook/search?k={}", intelx_url, api_key)
    } else {
        format!(
            "https://{}/phonebook/search/result?k={}&id={}&limit=100000",
            intelx_url,
            api_key,
            search_id.unwrap()
        )
    }
}

async fn get_searchid(host: Arc<String>) -> Result<String> {
    trace!("getting intelx searchid");

    let creds = match Creds::read_creds() {
        Ok(c) => c,
        Err(e) => return Err(e),
    };

    let client = reqwest::ClientBuilder::new()
        .timeout(Duration::from_secs(10))
        .pool_idle_timeout(Duration::from_secs(4))
        .build()?;

    let query_uri = build_url(&creds.url, &creds.api_key, true, None);
    let body = Query::new(host.to_string());
    let search_id: SearchId = client
        .post(&query_uri)
        .json(&body)
        .send()
        .await?
        .json()
        .await?;

    debug!("searchid: {:?}", &search_id);
    Ok(search_id.id)
}

pub async fn run(host: Arc<String>) -> Result<HashSet<String>> {
    trace!("fetching data from intelx for: {}", &host);
    let creds = match Creds::read_creds() {
        Ok(creds) => creds,
        Err(e) => return Err(e),
    };

    let search_id = get_searchid(host.clone()).await?;
    let uri = build_url(&creds.url, &creds.api_key, false, Some(&search_id));
    let resp: IntelxResults = reqwest::get(&uri).await?.json().await?;
    debug!("intelx response: {:?}", &resp);
    let subdomains = resp.subdomains();

    if !subdomains.is_empty() {
        Ok(subdomains)
    } else {
        Err(Error::source_error("Intelx", host))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore]
    async fn search_id() {
        let host = Arc::new("hackerone.com".to_owned());
        let id = get_searchid(host).await.unwrap();
        println!("{}", &id);
        assert!(id.len() > 0)
    }

    // Checks to see if the run function returns subdomains
    #[tokio::test]
    #[ignore]
    async fn returns_results() {
        let host = Arc::new("hackerone.com".to_owned());
        let results = run(host).await.unwrap();
        assert!(!results.is_empty());
    }

    #[tokio::test]
    #[ignore]
    async fn handle_no_results() {
        let host = Arc::new("anVubmxpa2VzdGVh.com".to_string());
        let res = run(host).await;
        let e = res.unwrap_err();
        assert_eq!(
            e.to_string(),
            "Intelx couldn't find any results for: anVubmxpa2VzdGVh.com"
        );
    }
}
