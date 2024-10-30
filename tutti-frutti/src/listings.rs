use crate::errors::FetchListingsError;
use futures::future;
use reqwest::Client;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;

#[derive(Debug)]
pub struct ListingNode {
    // Fields that define a ListingNode. Placeholder example:
    pub id: u32,
    pub name: String,
}

async fn init_session(
    client: &reqwest::Client,
    cookie_store: &reqwest::cookie::Jar,
) -> Result<String, FetchListingsError> {
    // Simulate session initialization and CSRF token fetching.
    // Placeholder implementation:
    Ok("dummy_csrf_token".to_string())
}

async fn perform_request(
    client: &reqwest::Client,
    csrf_token: &str,
    search_query: &str,
    offset: u32,
) -> Result<(u32, Vec<ListingNode>), FetchListingsError> {
    // Simulate fetching data.
    // Placeholder implementation:
    let listing = ListingNode {
        id: offset,
        name: format!("Listing {}", offset),
    };
    Ok((100, vec![listing]))
}

const FIRST: u32 = 20; // Example value for number of listings per page

/// Configuration for fetching listings.
pub struct SearchConfig {
    /// Maximum number of pages to fetch.
    pub max_pages: usize,
    /// Timeout in seconds for each request.
    pub timeout_secs: u64,
}

pub async fn fetch_listings(
    search_query: &str,
    config: SearchConfig,
) -> Result<Vec<ListingNode>, FetchListingsError> {
    let cookie_store = Arc::new(reqwest::cookie::Jar::default());
    let client = Client::builder()
        .cookie_provider(cookie_store.clone())
        .build()?;

    let csrf_token = init_session(&client, &cookie_store).await.map_err(|e| {
        FetchListingsError::CsrfTokenError(format!("Failed to initialize session: {}", e))
    })?;

    let (total_count, first_page_listings) =
        perform_request(&client, &csrf_token, search_query, 0).await?;

    let mut all_listings = first_page_listings;
    let total_pages = ((total_count + FIRST - 1) / FIRST) as usize;
    let total_pages = total_pages.min(config.max_pages);

    // Fetch remaining pages concurrently with timeout
    let mut tasks = vec![];
    for page in 1..total_pages {
        let offset = page as u32 * FIRST;
        let client = client.clone();
        let csrf_token = csrf_token.clone();
        let search_query = search_query.to_string();
        let timeout_duration = Duration::from_secs(config.timeout_secs);

        tasks.push(tokio::spawn(async move {
            timeout(
                timeout_duration,
                perform_request(&client, &csrf_token, &search_query, offset),
            )
            .await
        }));
    }

    let results = future::join_all(tasks).await;
    for result in results {
        match result {
            Ok(Ok(Ok((_, listings)))) => all_listings.extend(listings),
            Ok(Ok(Err(e))) => return Err(e), // Already a `FetchListingsError`
            Ok(Err(_)) => return Err(FetchListingsError::TimeoutError),
            Err(_) => return Err(FetchListingsError::TimeoutError),
        }
    }

    Ok(all_listings)
}
