use std::{
    cmp::Ordering,
    env,
    fs::{self, DirEntry, OpenOptions},
    io::{self, Write},
    path::Path,
    str::FromStr,
    time::{Duration, Instant},
};

use anyhow::Result;
use clap::{arg, command};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{prelude::*, widgets::*};

struct StatefulList<T> {
    state: ListState,
    items: Vec<T>,
}

impl<T> StatefulList<T> {
    fn with_items(items: Vec<T>, selected: Option<usize>) -> StatefulList<T> {
        StatefulList {
            state: ListState::default().with_selected(selected),
            items,
        }
    }

    fn selected(&self) -> Option<&T> {
        if let Some(idx) = self.state.selected() {
            self.items.get(idx)
        } else {
            None
        }
    }

    fn next(&mut self) {
        let i = self.state.selected().unwrap_or(0);
        if i < self.items.len() - 1 {
            self.state.select(Some(i + 1))
        }
    }

    fn previous(&mut self) {
        let i = self.state.selected().unwrap_or(0);
        if i > 0 {
            self.state.select(Some(i - 1));
        }
    }

    fn first(&mut self) {
        self.state.select(Some(0));
    }

    fn last(&mut self) {
        self.state.select(Some(self.items.len() - 1));
    }
}

fn read_dir_sorted(path: &Path, show_hidden: bool) -> Vec<DirEntry> {
    match fs::read_dir(path) {
        Ok(entries) => {
            let mut entries = entries
                .flatten()
                .filter(|x| {
                    !x.path().symlink_metadata().unwrap().is_symlink()
                        && (show_hidden || !x.file_name().to_string_lossy().starts_with('.'))
                })
                .collect::<Vec<_>>();
            entries.sort_by(|a, b| {
                let a = a.path();
                let b = b.path();
                let a_name = a.file_name().unwrap().to_string_lossy();
                let b_name = b.file_name().unwrap().to_string_lossy();
                if a.is_dir() && b.is_dir() {
                    a_name.cmp(&b_name)
                } else if a.is_dir() && !b.is_dir() {
                    Ordering::Less
                } else if !a.is_dir() && b.is_dir() {
                    Ordering::Greater
                } else {
                    a_name.cmp(&b_name)
                }
            });
            entries
        }
        Err(_) => vec![],
    }
}

#[derive(Debug)]
enum Mode {
    Normal,
    Find,
}

struct App {
    left: StatefulList<DirEntry>,
    center: StatefulList<DirEntry>,
    right: StatefulList<DirEntry>,
    mode: Mode,
    show_hidden: bool,
    show_icons: bool,
    accent: Color,
}

impl App {
    fn new(show_hidden: bool, show_icons: bool, accent: Color) -> App {
        let current_dir = env::current_dir().unwrap();
        let left = if let Some(parent) = current_dir.parent() {
            read_dir_sorted(parent, show_hidden)
        } else {
            Vec::new()
        };
        let center = read_dir_sorted(&current_dir, show_hidden);
        let right = if let Some(selected) = center.first() {
            read_dir_sorted(&current_dir.join(selected.path()), show_hidden)
        } else {
            Vec::new()
        };
        let left_selected = left.iter().position(|x| x.path().eq(current_dir.as_path()));

        App {
            left: StatefulList::with_items(left, left_selected),
            center: StatefulList::with_items(center, Some(0)),
            right: StatefulList::with_items(right, None),
            mode: Mode::Normal,
            show_hidden,
            show_icons,
            accent,
        }
    }

    fn enter(&mut self, path: &Path) {
        env::set_current_dir(path).unwrap();
        let left = read_dir_sorted(path.parent().unwrap(), self.show_hidden);
        let center = read_dir_sorted(path, self.show_hidden);
        let left_selected = left.iter().position(|x| x.path().eq(path));
        self.left = StatefulList::with_items(left, left_selected);
        self.center = StatefulList::with_items(center, Some(0));
    }

    fn leave(&mut self) {
        let leaving = env::current_dir().unwrap();
        if let Some(path) = leaving.parent() {
            env::set_current_dir(path).unwrap();
            let left = if let Some(parent) = path.parent() {
                read_dir_sorted(parent, self.show_hidden)
            } else {
                Vec::new()
            };
            let center = read_dir_sorted(path, self.show_hidden);
            let left_selected = left.iter().position(|x| x.path().eq(path));
            let center_selcted = center.iter().position(|x| x.path().eq(leaving.as_path()));
            self.left = StatefulList::with_items(left, left_selected);
            self.center = StatefulList::with_items(center, center_selcted);
        }
    }

    fn update_right(&mut self) {
        let current_dir = env::current_dir().unwrap();
        if let Some(selected) = self.center.selected() {
            let right = read_dir_sorted(&current_dir.join(selected.path()), self.show_hidden);
            self.right = StatefulList::with_items(right, None);
        }
    }
}

fn run_app<B: Backend>(
    terminal: &mut Terminal<B>,
    mut app: App,
    tick_rate: Duration,
) -> Result<()> {
    let mut last_tick = Instant::now();
    loop {
        terminal.draw(|f| ui(f, &mut app))?;

        let timeout = tick_rate.saturating_sub(last_tick.elapsed());
        if crossterm::event::poll(timeout)? {
            match event::read()? {
                Event::Key(key) => {
                    if key.kind == KeyEventKind::Press {
                        if matches!(app.mode, Mode::Normal) {
                            match key.code {
                                KeyCode::Esc => return Ok(()),
                                KeyCode::Down | KeyCode::Char('j') => {
                                    app.center.next();
                                    app.update_right();
                                }
                                KeyCode::Char('J') => {
                                    app.center.next();
                                    app.center.next();
                                    app.center.next();
                                    app.center.next();
                                    app.center.next();
                                    app.update_right();
                                }
                                KeyCode::Up | KeyCode::Char('k') => {
                                    app.center.previous();
                                    app.update_right();
                                }
                                KeyCode::Char('K') => {
                                    app.center.previous();
                                    app.center.previous();
                                    app.center.previous();
                                    app.center.previous();
                                    app.center.previous();
                                    app.update_right();
                                }
                                KeyCode::Char('g') => app.center.first(),
                                KeyCode::Char('G') => app.center.last(),
                                KeyCode::Left | KeyCode::Char('h') => {
                                    app.leave();
                                    app.update_right();
                                }
                                KeyCode::Right | KeyCode::Char('l') => {
                                    if let Some(selected) = app.center.selected() {
                                        if selected.path().is_dir() {
                                            app.enter(&selected.path());
                                            app.update_right();
                                        }
                                    }
                                }
                                KeyCode::Char('f') => app.mode = Mode::Find,
                                KeyCode::Char('q') | KeyCode::Enter => {
                                    if let Some(selected) = app.center.selected() {
                                        let path = selected.path();
                                        if path.is_dir() {
                                            fs::write("/tmp/seldir", path.to_str().unwrap())?;
                                            return Ok(());
                                        }
                                    }
                                }
                                _ => {}
                            }
                        } else {
                            if let KeyCode::Char(c) = key.code {
                                if let Mode::Find = app.mode {
                                    if let Some(idx) = app.center.items.iter().position(|x| {
                                        x.file_name()
                                            .into_string()
                                            .unwrap()
                                            .to_lowercase()
                                            .starts_with(c)
                                    }) {
                                        app.center.state.select(Some(idx));
                                    }
                                    app.update_right();
                                }
                            }

                            app.mode = Mode::Normal;
                        }
                    }
                }
                Event::Resize(_, _) => terminal.autoresize()?,
                _ => {}
            }
        }
        if last_tick.elapsed() >= tick_rate {
            last_tick = Instant::now();
        }
    }
}

fn into_list_item<'a>(dir_entry: &DirEntry, accent: Color, show_icons: bool) -> ListItem<'a> {
    let dir_icon = if show_icons { "   " } else { " " };
    let file_icon = if show_icons { "   " } else { " " };
    if dir_entry.metadata().unwrap().is_dir() {
        ListItem::new(dir_icon.to_owned() + dir_entry.file_name().to_str().unwrap())
            .style(Style::default().fg(accent))
    } else {
        ListItem::new(file_icon.to_owned() + dir_entry.file_name().to_str().unwrap())
    }
}

fn ui(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(f.size());

    let current_dir = env::current_dir().unwrap();
    let mut path = current_dir.to_str().unwrap().to_owned();
    if !path.ends_with('/') {
        path += "/";
    }
    let selection = if let Some(selected) = app.center.selected() {
        Span::from(selected.file_name().into_string().unwrap())
    } else {
        Span::default()
    };
    f.render_widget(
        Paragraph::new(Line::from(vec![Span::raw(path), selection])),
        chunks[0],
    );

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(20),
            Constraint::Percentage(30),
            Constraint::Percentage(30),
        ])
        .split(chunks[1]);

    let left: Vec<ListItem> = app
        .left
        .items
        .iter()
        .map(|x| into_list_item(x, app.accent, app.show_icons))
        .collect();
    let center: Vec<ListItem> = app
        .center
        .items
        .iter()
        .map(|x| into_list_item(x, app.accent, app.show_icons))
        .collect();
    let right: Vec<ListItem> = app
        .right
        .items
        .iter()
        .map(|x| into_list_item(x, app.accent, app.show_icons))
        .collect();

    let left = List::new(left)
        .highlight_style(Style::default().reversed())
        .block(Block::default().padding(Padding::new(0, 1, 0, 0)));
    let center = List::new(center)
        .highlight_style(Style::default().reversed())
        .block(Block::default().padding(Padding::new(0, 1, 0, 0)));
    let right = List::new(right).highlight_style(Style::default().reversed());

    f.render_stateful_widget(left, chunks[0], &mut app.left.state);
    f.render_stateful_widget(center, chunks[1], &mut app.center.state);
    f.render_stateful_widget(right, chunks[2], &mut app.right.state);
}

fn main() -> Result<()> {
    let matches = command!()
        .arg(arg!(-a --all "Show hidden files"))
        .arg(arg!(-i --icons "Show icons"))
        .arg(arg!(-c --color <COLOR> "Accent color"))
        .get_matches();

    let show_hidden = *matches.get_one::<bool>("all").unwrap();
    let show_icons = *matches.get_one::<bool>("icons").unwrap();
    let accent = matches
        .get_one::<String>("color")
        .unwrap_or(&"red".to_owned())
        .clone();

    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open("/tmp/seldir")
        .unwrap();
    write!(file, "{}", env::current_dir().unwrap().to_str().unwrap()).unwrap();

    crossterm::terminal::enable_raw_mode()?;
    let stdout = io::stdout();
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::with_options(
        backend,
        TerminalOptions {
            viewport: Viewport::Inline(16),
        },
    )?;
    let tick_rate = Duration::from_millis(200);
    let app = App::new(show_hidden, show_icons, Color::from_str(&accent)?);

    run_app(&mut terminal, app, tick_rate)?;

    crossterm::terminal::disable_raw_mode()?;
    terminal.clear()?;

    Ok(())
}
