use crate::graphql::{GraphQLResponse, ListingNode, FIRST, GRAPHQL_QUERY};
use crate::util::init_headers;
use reqwest::cookie::{CookieStore, Jar};
use reqwest::{
    header::{HeaderValue, CONTENT_TYPE, REFERER},
    Client,
};
use serde_json::json;
use std::error::Error;
use std::sync::Arc;
use uuid::Uuid;

pub async fn init_session(
    client: &Client,
    cookie_store: &Arc<Jar>,
) -> Result<String, Box<dyn Error + Send + Sync>> {
    client
        .get("https://www.tutti.ch")
        .headers(init_headers())
        .send()
        .await?;

    let url = "https://www.tutti.ch/".parse().unwrap();
    let cookies = cookie_store
        .cookies(&url)
        .map(|cookies| cookies.to_str().unwrap_or("").to_string())
        .unwrap_or_default();

    let csrf_token = cookies
        .split(';')
        .find_map(|cookie| {
            let cookie = cookie.trim();
            if cookie.starts_with("tutti_csrftoken=") {
                Some(cookie["tutti_csrftoken=".len()..].to_string())
            } else {
                None
            }
        })
        .ok_or("Failed to obtain CSRF token")?;

    Ok(csrf_token)
}

pub async fn perform_request(
    client: &Client,
    csrf_token: &str,
    search_query: &str,
    offset: u32,
) -> Result<(u32, Vec<ListingNode>), Box<dyn Error + Send + Sync>> {
    let x_tutti_hash = Uuid::new_v4().to_string();
    let current_date = chrono::Utc::now().format("%Y-%m-%d-%H-%M").to_string();
    let referer_hash = Uuid::new_v4().to_string().replace('-', "").to_lowercase();
    let encoded_query = urlencoding::encode(search_query);

    let variables = json!({
        "query": search_query,
        "constraints": null,
        "category": null,
        "first": FIRST,
        "offset": offset,
        "direction": "DESCENDING",
        "sort": "TIMESTAMP"
    });

    let payload = json!({
        "query": GRAPHQL_QUERY,
        "variables": variables
    });

    let mut headers = init_headers();
    headers.insert(
        REFERER,
        format!(
            "https://www.tutti.ch/de/q/suche/{}?sorting=newest&page=1&query={}",
            referer_hash, encoded_query
        )
        .parse()
        .unwrap(),
    );
    headers.insert(
        "X-Tutti-Hash",
        HeaderValue::from_str(&x_tutti_hash).unwrap(),
    );
    headers.insert(
        "X-Tutti-Source",
        format!("web r1.0-{}", current_date).parse().unwrap(),
    );
    headers.insert(
        "X-Tutti-Client-Identifier",
        format!(
            "web/1.0.0+env-live.git-{}",
            &x_tutti_hash.replace('-', "")[..8]
        )
        .parse()
        .unwrap(),
    );
    headers.insert("x-csrf-token", HeaderValue::from_str(csrf_token).unwrap());
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

    let response = client
        .post("https://www.tutti.ch/api/v10/graphql")
        .headers(headers)
        .json(&payload)
        .send()
        .await?
        .json::<GraphQLResponse>()
        .await?;

    // Handle errors in the response
    if let Some(errors) = response.errors {
        return Err(format!("API returned errors: {}", errors).into());
    }

    let data = response
        .data
        .ok_or("Empty data in response")?
        .searchListingsByQuery
        .listings;

    let total_count = data.totalCount;
    let listings = data
        .edges
        .into_iter()
        .map(|edge| edge.node)
        .collect::<Vec<_>>();

    Ok((total_count, listings))
}
