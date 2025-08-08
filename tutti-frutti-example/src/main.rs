use tutti_frutti::fetch_listings; // Import the function from your crate

#[tokio::main]
async fn main() {
    println!("{:#?}", fetch_listings("hometrainer").await);
}
