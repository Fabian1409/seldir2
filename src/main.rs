use std::{
    cmp::Ordering,
    env,
    error::Error,
    fs::{self, DirEntry, OpenOptions},
    io::{self, Write},
    path::Path,
    time::{Duration, Instant},
};

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
        let i = match self.state.selected() {
            Some(i) => {
                if i < self.items.len() - 1 {
                    i + 1
                } else {
                    i
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }

    fn previous(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if i != 0 {
                    i - 1
                } else {
                    0
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
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
}

impl App {
    fn new(show_hidden: bool, show_icons: bool) -> App {
        let current_dir = env::current_dir().unwrap();
        let left_items = read_dir_sorted(current_dir.parent().unwrap(), show_hidden);
        let center_items = read_dir_sorted(&current_dir, show_hidden);
        let right_items = read_dir_sorted(
            &current_dir.join(center_items.first().unwrap().path()),
            show_hidden,
        );
        let left_selected = left_items
            .iter()
            .position(|x| x.path().eq(current_dir.as_path()));

        App {
            left: StatefulList::with_items(left_items, left_selected),
            center: StatefulList::with_items(center_items, Some(0)),
            right: StatefulList::with_items(right_items, None),
            mode: Mode::Normal,
            show_hidden,
            show_icons,
        }
    }

    fn cd(&mut self) {
        let current_dir = env::current_dir().unwrap();
        let left_items = read_dir_sorted(current_dir.parent().unwrap(), self.show_hidden);
        let center_items = read_dir_sorted(&current_dir, self.show_hidden);
        let right_items = read_dir_sorted(
            &current_dir.join(
                center_items
                    .get(self.center.state.selected().unwrap())
                    .unwrap_or(center_items.first().unwrap())
                    .path(),
            ),
            self.show_hidden,
        );
        let left_selected = left_items
            .iter()
            .position(|x| x.path().eq(current_dir.as_path()));
        self.left = StatefulList::with_items(left_items, left_selected);
        self.center = StatefulList::with_items(center_items, Some(0));
        self.right = StatefulList::with_items(right_items, None);
    }

    fn update(&mut self) {
        let current_dir = env::current_dir().unwrap();
        let right_items = read_dir_sorted(
            &current_dir.join(self.center.selected().unwrap().path()),
            false,
        );
        self.right = StatefulList::with_items(right_items, None);
    }
}

fn run_app<B: Backend>(
    terminal: &mut Terminal<B>,
    mut app: App,
    tick_rate: Duration,
) -> Result<(), Box<dyn Error>> {
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
                                    app.update();
                                }
                                KeyCode::Char('J') => {
                                    app.center.next();
                                    app.center.next();
                                    app.center.next();
                                    app.center.next();
                                    app.center.next();
                                    app.update();
                                }
                                KeyCode::Up | KeyCode::Char('k') => {
                                    app.center.previous();
                                    app.update();
                                }
                                KeyCode::Char('K') => {
                                    app.center.previous();
                                    app.center.previous();
                                    app.center.previous();
                                    app.center.previous();
                                    app.center.previous();
                                    app.update();
                                }
                                KeyCode::Char('g') => app.center.first(),
                                KeyCode::Char('G') => app.center.last(),
                                KeyCode::Left | KeyCode::Char('h') => {
                                    env::set_current_dir(
                                        env::current_dir().unwrap().parent().unwrap(),
                                    )
                                    .unwrap();
                                    app.cd()
                                }
                                KeyCode::Right | KeyCode::Char('l') => {
                                    env::set_current_dir(
                                        env::current_dir()
                                            .unwrap()
                                            .join(app.center.selected().unwrap().path()),
                                    )
                                    .unwrap();
                                    app.cd()
                                }
                                KeyCode::Char('f') => app.mode = Mode::Find,
                                KeyCode::Char('q') | KeyCode::Enter => {
                                    fs::write(
                                        "/tmp/seldir",
                                        app.center.selected().unwrap().path().to_str().unwrap(),
                                    )?;
                                    return Ok(());
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
                                    app.update();
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

fn into_list_item<'a>(dir_entry: &DirEntry) -> ListItem<'a> {
    if dir_entry.metadata().unwrap().is_dir() {
        ListItem::new(String::from("   ") + dir_entry.file_name().to_str().unwrap())
            .style(Style::default().fg(Color::Red))
    } else {
        ListItem::new(String::from(" 󰈔  ") + dir_entry.file_name().to_str().unwrap())
            .style(Style::default().fg(Color::White))
    }
}

fn ui(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(f.size());

    let current_dir = env::current_dir().unwrap();
    let path = Span::from(current_dir.to_str().unwrap());
    let selection = Span::from(
        app.center
            .selected()
            .unwrap()
            .file_name()
            .into_string()
            .unwrap(),
    );
    f.render_widget(
        Paragraph::new(Line::from(vec![path, Span::raw("/"), selection])),
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

    let left_items: Vec<ListItem> = app.left.items.iter().map(into_list_item).collect();
    let center_items: Vec<ListItem> = app.center.items.iter().map(into_list_item).collect();
    let right_items: Vec<ListItem> = app.right.items.iter().map(into_list_item).collect();

    let left_items = List::new(left_items)
        .highlight_style(Style::default().reversed())
        .block(Block::default().padding(Padding::new(0, 1, 0, 0)));
    let center_items = List::new(center_items)
        .highlight_style(Style::default().reversed())
        .block(Block::default().padding(Padding::new(0, 1, 0, 0)));
    let right_items = List::new(right_items).highlight_style(Style::default().reversed());

    f.render_stateful_widget(left_items, chunks[0], &mut app.left.state);
    f.render_stateful_widget(center_items, chunks[1], &mut app.center.state);
    f.render_stateful_widget(right_items, chunks[2], &mut app.right.state);
}

fn main() -> Result<(), Box<dyn Error>> {
    let matches = command!()
        .arg(arg!(-a --all "Show hidden files"))
        .arg(arg!(-i --icons "Show icons"))
        .get_matches();

    let show_hidden = *matches.get_one::<bool>("all").unwrap();
    let show_icons = *matches.get_one::<bool>("icons").unwrap();

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
            viewport: Viewport::Inline(12),
        },
    )?;
    let tick_rate = Duration::from_millis(200);
    let app = App::new(show_hidden, show_icons);

    run_app(&mut terminal, app, tick_rate)?;

    crossterm::terminal::disable_raw_mode()?;
    terminal.clear()?;

    Ok(())
}
