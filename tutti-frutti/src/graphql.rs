use serde::{Deserialize, Serialize};

pub const FIRST: u32 = 30;

pub const GRAPHQL_QUERY: &str = r#"
query SearchListingsByConstraints($query: String, $constraints: ListingSearchConstraints, $category: ID, $first: Int!, $offset: Int!, $sort: ListingSortMode!, $direction: SortDirection!) {
  searchListingsByQuery(
    query: $query
    constraints: $constraints
    category: $category
  ) {
    listings(first: $first, offset: $offset, sort: $sort, direction: $direction) {
      totalCount
      edges {
        node {
          listingID
          title
          body
          timestamp
          formattedPrice
          sellerInfo {
            alias
          }
          thumbnail {
            normalRendition: rendition(width: 235, height: 167) {
              src
            }
          }
        }
      }
    }
  }
}
"#;

#[derive(Serialize, Deserialize, Debug)]
pub struct GraphQLResponse {
    pub data: Option<GraphQLData>,
    pub errors: Option<serde_json::Value>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct GraphQLData {
    pub searchListingsByQuery: ListingsByQuery,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ListingsByQuery {
    pub listings: Listings,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Listings {
    pub totalCount: u32,
    pub edges: Vec<Edge>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Edge {
    pub node: ListingNode,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ListingNode {
    pub listingID: String,
    pub title: String,
    pub body: String,
    pub timestamp: String,
    pub formattedPrice: Option<String>,
    pub sellerInfo: SellerInfo,
    pub thumbnail: Option<Thumbnail>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SellerInfo {
    pub alias: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Thumbnail {
    pub normalRendition: Option<Rendition>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Rendition {
    pub src: String,
}
