#!/bin/bash

# Initialize global variables
FIRST=30       # Number of results per page
OFFSET=0       # Starting offset
TOTAL_COUNT=0  # Will be updated after the first request
COOKIE_JAR=""  # Will store path to temporary cookie file
CSRF_TOKEN=""  # Will store the CSRF token
SEARCH_QUERY="" # Will store the search query

function show_usage() {
    echo "Usage: $0 <search_query>"
    exit 1
}

function cleanup() {
    [ -f "$COOKIE_JAR" ] && rm -f "$COOKIE_JAR"
}

function handle_error() {
    local error_message="$1"
    echo "Error: $error_message" >&2
    cleanup
    exit 1
}

function init_graphql_query() {
    read -r -d '' GRAPHQL_QUERY << 'EOQ'
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
EOQ
    # Escape the query for JSON
    QUERY=$(echo "$GRAPHQL_QUERY" | awk '{printf "%s\\n", $0}' | sed 's/"/\\"/g')
}

function generate_uuid() {
    if command -v uuidgen &> /dev/null; then
        echo $(uuidgen)
    else
        echo $(cat /proc/sys/kernel/random/uuid)
    fi
}

function init_session() {
    # Create temporary cookie jar
    COOKIE_JAR=$(mktemp) || handle_error "Failed to create temporary file"
    
    # Get initial CSRF token
    local csrf_response
    csrf_response=$(curl 'https://www.tutti.ch' \
        -c "$COOKIE_JAR" \
        -s \
        -H 'User-Agent: Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:131.0) Gecko/20100101 Firefox/131.0' \
        -H 'Accept: text/html,application/xhtml+xml,application/xml;q=0.9,image/webp,*/*;q=0.8' \
        -H 'Accept-Language: de,en-US;q=0.7,en;q=0.3' \
        -H 'Accept-Encoding: gzip, deflate, br' \
        -H 'Connection: keep-alive' \
        -H 'Upgrade-Insecure-Requests: 1' \
        --compressed)
    
    # Extract CSRF token
    CSRF_TOKEN=$(grep 'tutti_csrftoken' "$COOKIE_JAR" | awk '{print $7}')
    [ -z "$CSRF_TOKEN" ] && handle_error "Failed to obtain CSRF token"
}

function perform_request() {
    local offset=$1
    local x_tutti_hash=$(generate_uuid)
    local current_date=$(date '+%Y-%m-%d-%H-%M')
    local referer_hash=$(generate_uuid | tr -d '-')
    local encoded_query=$(echo "$SEARCH_QUERY" | sed 's/ /+/g')
    
    # Prepare variables for the request
    local variables=$(jq -n \
        --arg query "$SEARCH_QUERY" \
        --argjson constraints null \
        --argjson category null \
        --argjson first "$FIRST" \
        --argjson offset "$offset" \
        --arg direction "DESCENDING" \
        --arg sort "TIMESTAMP" \
        '{
            query: $query,
            constraints: $constraints,
            category: $category,
            first: $first,
            offset: $offset,
            direction: $direction,
            sort: $sort
        }')
    
    # Construct the JSON payload
    local payload="{\"query\":\"$QUERY\",\"variables\":$variables}"
    
    # Make the GraphQL request with all headers inline
    local response
    response=$(curl 'https://www.tutti.ch/api/v10/graphql' \
        -X POST \
        -H "User-Agent: Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:131.0) Gecko/20100101 Firefox/131.0" \
        -H "Accept: */*" \
        -H "Accept-Language: de,en-US;q=0.7,en;q=0.3" \
        -H "Accept-Encoding: gzip, deflate, br, zstd" \
        -H "Referer: https://www.tutti.ch/de/q/suche/${referer_hash}?sorting=newest&page=1&query=${encoded_query}" \
        -H "Content-Type: application/json" \
        -H "X-Tutti-Hash: ${x_tutti_hash}" \
        -H "X-Tutti-Source: web r1.0-${current_date}" \
        -H "X-Tutti-Client-Identifier: web/1.0.0+env-live.git-$(echo $x_tutti_hash | cut -c1-8)" \
        -H "x-csrf-token: ${CSRF_TOKEN}" \
        -H "Origin: https://www.tutti.ch" \
        -H "Connection: keep-alive" \
        -H "Sec-Fetch-Dest: empty" \
        -H "Sec-Fetch-Mode: cors" \
        -H "Sec-Fetch-Site: same-origin" \
        -H "Sec-GPC: 1" \
        -H "Priority: u=0" \
        -H "TE: trailers" \
        -b "$COOKIE_JAR" \
        --compressed \
        --data-raw "$payload" \
        -s)
    
    # Validate response
    [ -z "$response" ] && handle_error "Empty response received"
    
    # Check if the response is valid JSON
    echo "$response" | jq . >/dev/null 2>&1 || handle_error "Invalid JSON response received"
    
    local errors=$(echo "$response" | jq '.errors')
    [ "$errors" != "null" ] && handle_error "API returned errors: $errors"
    
    # Update total count if this is the first request
    if [ "$TOTAL_COUNT" -eq 0 ]; then
        TOTAL_COUNT=$(echo "$response" | jq '.data.searchListingsByQuery.listings.totalCount')
        [ -z "$TOTAL_COUNT" ] || [ "$TOTAL_COUNT" = "null" ] && handle_error "Failed to get total count from response"
    fi
    
    # Output the listings
    echo "$response" | jq -c '.data.searchListingsByQuery.listings.edges[].node'
}

function fetch_page_in_background() {
    local offset="$1"
    perform_request "$offset" &
}

function main() {
    # Validate input
    [ "$#" -lt 1 ] && show_usage
    SEARCH_QUERY="$1"
    
    # Set up trap for cleanup
    trap cleanup EXIT
    
    # Initialize everything
    init_graphql_query
    init_session
    
    # Perform initial request to get total count
    perform_request "$OFFSET"
    
    # Calculate and process remaining pages in parallel
    local total_pages=$(( (TOTAL_COUNT + FIRST - 1) / FIRST ))
    for (( page=1; page<total_pages; page++ )); do
        OFFSET=$(( page * FIRST ))
        fetch_page_in_background "$OFFSET"
    done

    # Wait for all background jobs to finish
    wait
}

# Start the script
main "$@"
