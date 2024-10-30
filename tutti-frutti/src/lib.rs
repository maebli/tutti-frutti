pub mod client;
pub mod graphql;
pub mod util;

use client::{init_session, perform_request};
use graphql::{ListingNode, FIRST};
use reqwest::Client;
use std::error::Error;
use std::sync::Arc;

pub async fn fetch_listings(
    search_query: &str,
) -> Result<Vec<ListingNode>, Box<dyn Error + Send + Sync>> {
    let cookie_store = Arc::new(reqwest::cookie::Jar::default());
    let client = Client::builder()
        .cookie_provider(cookie_store.clone())
        .build()?;

    let csrf_token = init_session(&client, &cookie_store).await?;

    let (total_count, first_page_listings) =
        perform_request(&client, &csrf_token, search_query, 0).await?;

    let mut all_listings = first_page_listings;
    let total_pages = ((total_count + FIRST - 1) / FIRST) as usize;

    // Fetch remaining pages concurrently
    let mut tasks = vec![];
    for page in 1..total_pages {
        let offset = page as u32 * FIRST;
        let client = client.clone();
        let csrf_token = csrf_token.clone();
        let search_query = search_query.to_string();

        tasks.push(tokio::spawn(async move {
            perform_request(&client, &csrf_token, &search_query, offset).await
        }));
    }

    let results = futures::future::join_all(tasks).await;
    for result in results {
        let (_, listings) = result??;
        all_listings.extend(listings);
    }

    Ok(all_listings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fetch_listings;

    #[tokio::test]
    async fn test_fetch_listings_with_pencil_query() {
        let query = "pencil";
        let result = fetch_listings(query).await;

        match result {
            Ok(listings) => {
                assert!(
                    !listings.is_empty(),
                    "The listings should not be empty for query: {}",
                    query
                );
                println!("Found {} listings for query '{}'", listings.len(), query);
                for listing in listings.iter() {
                    println!("{:?}", listing);
                }
            }
            Err(err) => panic!("Failed to fetch listings for query '{}': {:?}", query, err),
        }
    }
}
