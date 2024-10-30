# Tutti-Frutti

## Introduction
<img src="tuttifrutti.png" alt="Tutti Frutti" width="100"/>

This is a small Rust lib that will fetch all search results for a query from `tutti.ch`.
Use with caution and make sure you follow most current ToS of `tutti.ch`. 

Input: `query string`
Output: String List of entries: `{"listingID":"","title":"","body":"","timestamp":"","formattedPrice":"","sellerInfo":{"alias":""},"thumbnail":{"normalRendition":{"src":""}}}`

Test shows how it works:

``` rust
    use tutti_frutti::fetch_listings; // Import the function from your crate

    #[tokio::main]
    async fn main() {
        println!("{:#?}", fetch_listings("tutti frutti").await);
    }
```

## Overview

- `tutti-frutti.sh`-> same thing but in bash
- `tutti-frutti/`-> folder containing the lib
- `tutti-frutti-example/`-> example using the lib

## Using the Lib

To run a query do the following

1. Install Rust: https://www.rust-lang.org/tools/install
2. run `cargo run --release -p tutti-frutti-example`

