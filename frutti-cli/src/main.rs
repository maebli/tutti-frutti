use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Terminal,
};
use std::{io, time::Duration};
use tutti_frutti::{fetch_listings, graphql::ListingNode};

// Define an enum for sort categories
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SortCategory {
    Default,
    Title,
    Price,
    Seller,
}

impl SortCategory {
    fn next(&self) -> Self {
        match self {
            SortCategory::Default => SortCategory::Title,
            SortCategory::Title => SortCategory::Price,
            SortCategory::Price => SortCategory::Seller,
            SortCategory::Seller => SortCategory::Default,
        }
    }
    
    fn as_str(&self) -> &'static str {
        match self {
            SortCategory::Default => "Default",
            SortCategory::Title => "Title",
            SortCategory::Price => "Price",
            SortCategory::Seller => "Seller",
        }
    }
}

// New struct to store price statistics
struct PriceStats {
    count: usize,
    min: f64,
    max: f64,
    mean: f64,
    median: f64,
    histogram: Vec<usize>,
    bin_width: f64,
}

struct App {
    listings: Vec<ListingNode>,
    list_state: ListState,
    search_query: String,
    search_mode: bool,
    loading: bool,
    error: Option<String>,
    sort_category: SortCategory,
    stats_mode: bool,  // New field to track stats mode
}

impl App {
    fn new() -> App {
        App {
            listings: Vec::new(),
            list_state: ListState::default(),
            search_query: String::from("tutti frutti"),
            search_mode: false,
            loading: false,
            error: None,
            sort_category: SortCategory::Default,
            stats_mode: false,
        }
    }

    fn next(&mut self) {
        let i = match self.list_state.selected() {
            Some(i) => {
                if i >= self.listings.len().saturating_sub(1) {
                    0
                } else {
                    i + 1
                }
            }
            None if !self.listings.is_empty() => 0,
            None => return,
        };
        self.list_state.select(Some(i));
    }

    fn previous(&mut self) {
        let i = match self.list_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.listings.len().saturating_sub(1)
                } else {
                    i - 1
                }
            }
            None if !self.listings.is_empty() => 0,
            None => return,
        };
        self.list_state.select(Some(i));
    }

    fn toggle_sort(&mut self) {
        self.sort_category = self.sort_category.next();
        self.sort_listings();
    }

    fn sort_listings(&mut self) {
        // Remember the currently selected item if any
        let selected_index = self.list_state.selected();
        let selected_id = selected_index.and_then(|i| 
            self.listings.get(i).map(|item| item.listingID.clone())
        );
        
        match self.sort_category {
            SortCategory::Default => {
                // Keep original order from API
            },
            SortCategory::Title => {
                self.listings.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));
            },
            SortCategory::Price => {
                // Parse prices and sort numerically
                self.listings.sort_by(|a, b| {
                    let parse_price = |price: &Option<String>| -> Option<f64> {
                        price
                            .as_ref()
                            .and_then(|p| p.replace(['C', 'H', 'F', ',', ' '], "").parse::<f64>().ok())
                    };
                    
                    let price_a = parse_price(&a.formattedPrice);
                    let price_b = parse_price(&b.formattedPrice);
                    
                    match (price_a, price_b) {
                        (Some(a), Some(b)) => a.partial_cmp(&b).unwrap_or(std::cmp::Ordering::Equal),
                        (Some(_), None) => std::cmp::Ordering::Less,
                        (None, Some(_)) => std::cmp::Ordering::Greater,
                        (None, None) => std::cmp::Ordering::Equal,
                    }
                });
            },
            SortCategory::Seller => {
                self.listings.sort_by(|a, b| {
                    a.sellerInfo.alias.to_lowercase().cmp(&b.sellerInfo.alias.to_lowercase())
                });
            }
        }

        // Restore selection after sorting
        if let Some(id) = selected_id {
            if let Some(new_index) = self.listings.iter().position(|item| item.listingID == id) {
                self.list_state.select(Some(new_index));
            } else if !self.listings.is_empty() {
                self.list_state.select(Some(0));
            }
        }
    }

    async fn search(&mut self, query: &str) -> Result<()> {
        // Validate query before searching
        if query.trim().is_empty() {
            self.error = Some("Search query cannot be empty".to_string());
            return Ok(());
        }

        self.loading = true;
        self.error = None;
        
        // Use a safer error-handling approach
        let result = match fetch_listings(query).await {
            Ok(listings) => {
                self.listings = listings;
                if !self.listings.is_empty() {
                    self.list_state.select(Some(0));
                    // Apply current sort if not default
                    if self.sort_category != SortCategory::Default {
                        self.sort_listings();
                    }
                } else {
                    self.list_state.select(None);
                }
                Ok(())
            }
            Err(e) => {
                self.error = Some(format!("Search error: {}", e));
                self.listings = Vec::new();
                self.list_state.select(None);
                Ok(())
            }
        };
        
        self.loading = false;
        result
    }

    // Add a new function to construct and open the listing URL
    fn open_selected_listing(&self) -> Result<()> {
        if let Some(selected) = self.list_state.selected() {
            if let Some(listing) = self.listings.get(selected) {
                let url = format!("https://www.tutti.ch/de/vi/{}", listing.listingID);
                println!("Opening: {}", url);
                open::that(url)?;
            }
        }
        Ok(())
    }

    // Add a function to toggle stats mode
    fn toggle_stats_mode(&mut self) {
        self.stats_mode = !self.stats_mode;
    }

    // Function to calculate price statistics
    fn calculate_price_stats(&self) -> PriceStats {
        // Extract prices and convert to numbers
        let mut prices: Vec<f64> = self.listings.iter()
            .filter_map(|listing| {
                listing.formattedPrice
                    .as_ref()
                    .and_then(|p| {
                        // More robust price parsing
                        // First, normalize to numeric characters and decimal point
                        let sanitized = p.chars()
                            .map(|c| match c {
                                '0'..='9' => c,
                                '.' | ',' => '.', // Convert both . and , to decimal point
                                _ => ' '          // Replace all other chars with spaces
                            })
                            .collect::<String>();
                        
                        // Remove all spaces
                        let cleaned = sanitized.replace(' ', "");
                        
                        // Try to parse as f64
                        cleaned.parse::<f64>().ok()
                    })
            })
            .collect();
        
        // Handle empty case
        if prices.is_empty() {
            return PriceStats {
                count: 0,
                min: 0.0,
                max: 0.0,
                mean: 0.0,
                median: 0.0,
                histogram: vec![0; 10],
                bin_width: 0.0,
            };
        }
        
        // Sort prices for median calculation
        prices.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        
        let count = prices.len();
        let min = prices.first().cloned().unwrap_or(0.0);
        let max = prices.last().cloned().unwrap_or(0.0);
        let sum: f64 = prices.iter().sum();
        let mean = if count > 0 { sum / count as f64 } else { 0.0 };
        
        // Calculate median
        let median = if count > 0 {
            if count % 2 == 0 {
                (prices[count / 2 - 1] + prices[count / 2]) / 2.0
            } else {
                prices[count / 2]
            }
        } else {
            0.0
        };
        
        // Create histogram with 10 bins
        let mut histogram = vec![0; 10];
        if count > 0 && max > min {
            // Calculate bin width
            let bin_width = (max - min) / 10.0;
            
            // Create explicit bin boundaries for more accurate distribution
            let bin_boundaries: Vec<f64> = (0..10)
                .map(|i| min + (i as f64 * bin_width))
                .collect();
            
            // Assign each price to a bin
            for price in prices.iter() {
                // Find the appropriate bin
                let mut bin_idx = 9; // Default to last bin
                for (i, boundary) in bin_boundaries.iter().enumerate() {
                    let upper_bound = if i < 9 { bin_boundaries[i + 1] } else { max + 0.01 }; // Add small value to include max
                    if *price >= *boundary && *price < upper_bound {
                        bin_idx = i;
                        break;
                    }
                }
                histogram[bin_idx] += 1;
            }
            
            return PriceStats {
                count,
                min,
                max,
                mean,
                median,
                histogram,
                bin_width,
            };
        } else {
            // If all prices are the same
            histogram[0] = count;
            return PriceStats {
                count,
                min,
                max,
                mean,
                median,
                histogram,
                bin_width: 1.0,
            };
        }
    }
}

// Add this helper function for safe string truncation
fn truncate_to_char_boundary(s: &str, max_chars: usize) -> &str {
    if s.chars().count() <= max_chars {
        return s;
    }

    let mut char_indices = s.char_indices();
    for _ in 0..max_chars {
        if let None = char_indices.next() {
            return s; // String is shorter than max_chars
        }
    }
    
    // Get the next character boundary
    if let Some((idx, _)) = char_indices.next() {
        &s[..idx]
    } else {
        s // This should not happen, but return the whole string just in case
    }
}

// Helper function to render price statistics
fn render_price_stats(stats: &PriceStats) -> Paragraph {
    let mut lines = vec![
        Line::from(vec![
            Span::styled("Price Statistics", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::raw(format!("Count: {} items with price information", stats.count)),
        ]),
        Line::from(vec![
            Span::raw(format!("Range: CHF {:.2} - CHF {:.2}", stats.min, stats.max)),
        ]),
        Line::from(vec![
            Span::raw(format!("Average: CHF {:.2}", stats.mean)),
        ]),
        Line::from(vec![
            Span::raw(format!("Median: CHF {:.2}", stats.median)),
        ]),
        Line::from(vec![
            Span::styled("Price Distribution:", Style::default().add_modifier(Modifier::BOLD)),
        ]),
    ];
    
    // Skip histogram if no data
    if stats.count > 0 {
        // Find the maximum count in the histogram for scaling
        let max_count = *stats.histogram.iter().max().unwrap_or(&1);
        
        // Add histogram bars
        for (i, &count) in stats.histogram.iter().enumerate() {
            let bin_start = stats.min + i as f64 * stats.bin_width;
            let bin_end = bin_start + stats.bin_width;
            
            let bin_label = format!("CHF {:.0}-{:.0}", bin_start, bin_end);
            let percent = count as f64 / max_count as f64;
            
            // Create a bar using Unicode block characters
            let bar_width = (40.0 * percent).round() as usize;
            let bar = "█".repeat(bar_width);
            
            lines.push(Line::from(vec![
                Span::raw(format!("{:<15} ", bin_label)),
                Span::styled(bar, Style::default().fg(Color::Blue)),
                Span::raw(format!(" {}", count)),
            ]));
        }
    } else {
        lines.push(Line::from("No price data available"));
    }
    
    Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title("Price Statistics"))
        .wrap(ratatui::widgets::Wrap { trim: false })
}

#[tokio::main]
async fn main() -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app state
    let mut app = App::new();
    
    // Initial search - FIX: Clone the query first
    let initial_query = app.search_query.clone();
    app.search(&initial_query).await?;

    // Main loop
    loop {
        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Min(0),
                    Constraint::Length(3),
                ])
                .margin(1)
                .split(f.size());

            // Search bar
            let search_style = if app.search_mode {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default()
            };
            
            let search_text = if app.search_mode {
                format!("{}", app.search_query)
            } else {
                format!("{} (press / to edit)", app.search_query)
            };
            
            let search_bar = Paragraph::new(search_text)
                .style(search_style)
                .block(Block::default().borders(Borders::ALL).title("Search"));
            
            f.render_widget(search_bar, chunks[0]);

            // Results area or stats view
            let results_block = Block::default()
                .borders(Borders::ALL)
                .title(format!("Results ({})", app.listings.len()));

            if app.loading {
                let loading = Paragraph::new("Loading...")
                    .block(results_block);
                f.render_widget(loading, chunks[1]);
            } else if let Some(ref error) = app.error {
                let error_text = Paragraph::new(error.as_str())
                    .style(Style::default().fg(Color::Red))
                    .block(results_block);
                f.render_widget(error_text, chunks[1]);
            } else if app.listings.is_empty() {
                let empty = Paragraph::new("No results found.")
                    .block(results_block);
                f.render_widget(empty, chunks[1]);
            } else if app.stats_mode {
                // Show price stats when in stats mode
                let stats = app.calculate_price_stats();
                let stats_view = render_price_stats(&stats);
                f.render_widget(stats_view, chunks[1]);
            } else {
                let items: Vec<ListItem> = app
                    .listings
                    .iter()
                    .map(|l| {
                        let price = l.formattedPrice.as_deref().unwrap_or("No price");
                        let seller = &l.sellerInfo.alias;
                        
                        // Get a truncated description that respects UTF-8 character boundaries
                        let truncated_body = truncate_to_char_boundary(&l.body, 50);
                        let ellipsis = if truncated_body.len() < l.body.len() { "..." } else { "" };
                        
                        ListItem::new(vec![
                            Line::from(vec![
                                Span::styled(&l.title, Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                            ]),
                            Line::from(vec![
                                Span::raw(format!("Price: {} | Seller: {}", price, seller)),
                            ]),
                            Line::from(vec![
                                Span::styled(truncated_body, Style::default().fg(Color::Gray)),
                                Span::raw(ellipsis),
                            ]),
                        ])
                    })
                    .collect();

                let listings = List::new(items)
                    .block(results_block)
                    .highlight_style(Style::default().bg(Color::DarkGray))
                    .highlight_symbol("> ");

                // First render the list widget
                let list_area = chunks[1];
                f.render_stateful_widget(listings, list_area, &mut app.list_state);
                
                // Then create and render a scrollbar
                // We need to calculate where to place the scrollbar
                if !app.listings.is_empty() {
                    use ratatui::widgets::{Scrollbar, ScrollbarOrientation, ScrollbarState};
                    
                    // Get inner height excluding the block borders
                    let inner_height = list_area.height.saturating_sub(2);
                    
                    // Create scrollbar state with proper type conversions
                    let total_items = app.listings.len(); // This is already usize
                    let position = app.list_state.selected().unwrap_or(0); // This is already usize
                    let scrollbar_state = ScrollbarState::new(total_items)
                        .position(position);
                    
                    let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
                        .begin_symbol(Some("↑"))
                        .end_symbol(Some("↓"))
                        .track_symbol(Some("│"))
                        .thumb_symbol("█")
                        .track_style(Style::default().fg(Color::DarkGray))
                        .thumb_style(Style::default().fg(Color::White));
                    
                    // Calculate scrollbar area (position it on the right edge of the list area)
                    let scrollbar_area = ratatui::layout::Rect {
                        x: list_area.x + list_area.width - 2, // Put it on the right edge
                        y: list_area.y + 1, // Skip the border
                        width: 1,
                        height: inner_height,
                    };
                    
                    f.render_stateful_widget(scrollbar, scrollbar_area, &mut scrollbar_state.clone());
                }
            }

            // Help bar
            let help_text = if app.search_mode {
                String::from("Enter: Submit Search | Esc: Cancel")
            } else if app.stats_mode {
                String::from("q: Quit | Esc/p: Back to Listings")
            } else {
                format!("q: Quit | j/Down: Next | k/Up: Previous | /: Search | s: Sort ({}) | p: Price Stats | Enter: Open",
                    app.sort_category.as_str())
            };
            
            let help_bar = Paragraph::new(help_text)
                .block(Block::default().borders(Borders::ALL).title("Help"));
            
            f.render_widget(help_bar, chunks[2]);
        })?;

        // Handle input
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if app.search_mode {
                    match key.code {
                        KeyCode::Enter => {
                            app.search_mode = false;
                            let query = app.search_query.clone();
                            // Only search if query isn't empty
                            if !query.trim().is_empty() {
                                match app.search(&query).await {
                                    Ok(_) => {},
                                    Err(e) => {
                                        app.error = Some(format!("Error during search: {}", e));
                                    }
                                }
                            } else {
                                app.error = Some("Search query cannot be empty".to_string());
                            }
                        }
                        KeyCode::Esc => {
                            app.search_mode = false;
                        }
                        KeyCode::Backspace => {
                            app.search_query.pop();
                        }
                        // Handle Ctrl+U to clear the query (fixed with proper modifier check)
                        KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            app.search_query.clear();
                        }
                        KeyCode::Char(c) => {
                            app.search_query.push(c);
                        }
                        _ => {}
                    }
                } else {
                    match key.code {
                        KeyCode::Char('q') => {
                            break;
                        }
                        KeyCode::Char('j') | KeyCode::Down => {
                            if !app.stats_mode {
                                app.next();
                            }
                        }
                        KeyCode::Char('k') | KeyCode::Up => {
                            if !app.stats_mode {
                                app.previous();
                            }
                        }
                        KeyCode::Char('p') => {
                            app.toggle_stats_mode();
                        }
                        KeyCode::Esc => {
                            if app.stats_mode {
                                app.stats_mode = false;
                            }
                        }
                        KeyCode::Char('/') => {
                            app.search_mode = true;
                            app.search_query.clear();
                        }
                        KeyCode::Char('s') => {
                            app.toggle_sort();
                        }
                        KeyCode::Enter => {
                            // Open the selected listing in browser when Enter is pressed
                            if let Err(e) = app.open_selected_listing() {
                                app.error = Some(format!("Failed to open browser: {}", e));
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}
