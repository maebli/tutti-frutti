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

struct App {
    listings: Vec<ListingNode>,
    list_state: ListState,
    search_query: String,
    search_mode: bool,
    loading: bool,
    error: Option<String>,
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

            // Results area
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

                f.render_stateful_widget(listings, chunks[1], &mut app.list_state);
            }

            // Help bar
            let help_text = if app.search_mode {
                "Enter: Submit Search | Esc: Cancel"
            } else {
                "q: Quit | j/Down: Next | k/Up: Previous | /: Search"
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
                            app.next();
                        }
                        KeyCode::Char('k') | KeyCode::Up => {
                            app.previous();
                        }
                        KeyCode::Char('/') => {
                            app.search_mode = true;
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
