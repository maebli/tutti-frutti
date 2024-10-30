use reqwest::header::{
    HeaderMap, HeaderValue, ACCEPT, ACCEPT_ENCODING, ACCEPT_LANGUAGE, CONNECTION, ORIGIN,
    USER_AGENT,
};

pub fn init_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(USER_AGENT, HeaderValue::from_static("Mozilla/5.0"));
    headers.insert(ACCEPT, HeaderValue::from_static("*/*"));
    headers.insert(
        ACCEPT_LANGUAGE,
        HeaderValue::from_static("de,en-US;q=0.7,en;q=0.3"),
    );
    headers.insert(
        ACCEPT_ENCODING,
        HeaderValue::from_static("gzip, deflate, br, zstd"),
    );
    headers.insert(ORIGIN, HeaderValue::from_static("https://www.tutti.ch"));
    headers.insert(CONNECTION, HeaderValue::from_static("keep-alive"));
    headers
}
