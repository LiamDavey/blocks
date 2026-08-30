use std::io;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::{
    DefaultTerminal, Frame,
    layout::Rect,
    layout::{Alignment, Constraint},
    style::{self, Color},
    text::Span,
    widgets::{Block, Clear, Paragraph},
};

// W = Wall
// P = Player
// B = Box
// S = Storage
// F = Filled Storage
#[rustfmt::skip]
const MAPS: &[&[&str]] = &[
    &[
        "WWWWWWWW",
        "W------W",
        "W-P-B-SW",
        "W---B-SW",
        "W------W",
        "WWWWWWWW",
    ],
    &[
        "--WWWWW",
        "--WPW-W",
        "WWWBS-W",
        "W-B---W",
        "W-S-BSW",
        "W-----W",
        "WWWWWWW",
    ],
    &[
        "--WWWWW-",
        "WWW---W-",
        "WSPB--W-",
        "WWW-BSW-",
        "WSWWB-W-",
        "W-W-S-WW",
        "WB-FBBSW",
        "W---S--W",
        "WWWWWWWW",
    ],
];

fn main() -> io::Result<()> {
    ratatui::run(|terminal| Game::new().run(terminal))
}

#[derive(Clone, Copy)]
pub enum Tile {
    Wall,
    Box,
    Storage,
    Filled,
    Empty,
}

impl Tile {
    fn symbol(&self) -> &str {
        match self {
            Self::Storage => "\u{f444}",
            _ => "██",
        }
    }
    fn colour(&self) -> Color {
        match self {
            Self::Wall => Color::DarkGray,
            Self::Box | Self::Filled => Color::Blue,
            Self::Storage => Color::LightBlue,
            Self::Empty => Color::Reset,
        }
    }
}

#[derive(Clone, Copy)]
struct Coord {
    x: u16,
    y: u16,
}

impl Coord {
    fn step(&self, direction: Direction) -> Coord {
        let (x, y) = match direction {
            Direction::Up => (self.x, self.y.saturating_sub(1)),
            Direction::Down => (self.x, self.y + 1),
            Direction::Left => (self.x.saturating_sub(1), self.y),
            Direction::Right => (self.x + 1, self.y),
        };
        Coord { x, y }
    }
}

struct Player {
    position: Coord,
    symbol: &'static str,
    colour: style::Color,
}

impl Player {
    fn new(x: u16, y: u16) -> Self {
        Self {
            position: Coord { x, y },
            symbol: "██",
            colour: Color::Cyan,
        }
    }
}

#[derive(Clone, Copy)]
enum Direction {
    Up,
    Down,
    Left,
    Right,
}

pub struct Level {
    grid: Vec<Vec<Tile>>,
    player: Player,
    won: bool,
}

impl Level {
    pub fn from_map(map: &[&str]) -> Self {
        let mut grid = vec![];
        let mut player = None;
        for (y, line) in map.iter().enumerate() {
            let mut row = vec![];
            for (x, c) in line.chars().enumerate() {
                let block = match c {
                    'W' => Tile::Wall,
                    'B' => Tile::Box,
                    'P' => {
                        player = Some(Player::new(x as u16, y as u16));
                        Tile::Empty
                    }
                    'S' => Tile::Storage,
                    'F' => Tile::Filled,
                    '-' => Tile::Empty,
                    _ => panic!("unknown map character: {c}"),
                };
                row.push(block);
            }
            grid.push(row);
        }
        let player = player.expect("all maps must have a player");
        Self {
            grid,
            player,
            won: false,
        }
    }

    pub fn is_won(&self) -> bool {
        !self
            .grid
            .iter()
            .flatten()
            .any(|tile| matches!(tile, Tile::Storage))
    }

    fn check_coord(&self, coord: Coord) -> Tile {
        self.grid[coord.y as usize][coord.x as usize]
    }

    fn set_coord(&mut self, tile: Tile, coord: Coord) {
        self.grid[coord.y as usize][coord.x as usize] = tile;
    }

    fn try_move_player(&mut self, direction: Direction) {
        let next_step = self.player.position.step(direction);
        match self.check_coord(next_step) {
            Tile::Empty | Tile::Storage => {
                self.player.position = next_step;
            }
            Tile::Wall => {}
            Tile::Box => {
                let next_next_step = next_step.step(direction);
                match self.check_coord(next_next_step) {
                    Tile::Empty => {
                        self.player.position = next_step;
                        self.set_coord(Tile::Empty, next_step);
                        self.set_coord(Tile::Box, next_next_step);
                    }
                    Tile::Storage => {
                        self.player.position = next_step;
                        self.set_coord(Tile::Empty, next_step);
                        self.set_coord(Tile::Filled, next_next_step);
                        if self.is_won() {
                            self.won = true;
                        }
                    }
                    _ => {}
                }
            }
            Tile::Filled => {
                let next_next_step = next_step.step(direction);
                match self.check_coord(next_next_step) {
                    Tile::Empty => {
                        self.player.position = next_step;
                        self.set_coord(Tile::Storage, next_step);
                        self.set_coord(Tile::Box, next_next_step);
                    }
                    Tile::Storage => {
                        self.player.position = next_step;
                        self.set_coord(Tile::Storage, next_step);
                        self.set_coord(Tile::Filled, next_next_step);
                    }
                    _ => {}
                }
            }
        }
    }
    fn map_width(&self) -> usize {
        // each grid is 2 chars wide cause it looks better
        self.grid.first().unwrap().len() * 2
    }
    fn map_height(&self) -> usize {
        self.grid.len()
    }
}

fn centered_rect_origin(frame: &Frame, width: usize, height: usize) -> Coord {
    // Center a rectangle, and use the top left corner as the origin
    let centered_map_area = frame.area().centered(
        Constraint::Length(width as u16),
        Constraint::Length(height as u16),
    );
    Coord {
        x: centered_map_area.x,
        y: centered_map_area.y,
    }
}

pub struct Game {
    map_idx: usize,
    level: Level,
    exit: bool,
}

impl Default for Game {
    fn default() -> Self {
        Self::new()
    }
}

impl Game {
    pub fn new() -> Self {
        let level = Level::from_map(MAPS[0]);
        Self {
            map_idx: 0,
            level,
            exit: false,
        }
    }

    /// runs the application's main loop until the user quits
    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
        while !self.exit {
            terminal.draw(|frame| self.draw(frame))?;
            self.handle_events()?;
        }
        Ok(())
    }

    fn next_level(&mut self) {
        if !self.last_level() {
            self.map_idx += 1;
            self.level = Level::from_map(MAPS[self.map_idx]);
        }
    }

    fn last_level(&self) -> bool {
        self.map_idx == MAPS.len() - 1
    }

    fn restart_level(&mut self) {
        self.level = Level::from_map(MAPS[self.map_idx]);
    }

    fn draw(&self, frame: &mut Frame) {
        let origin = centered_rect_origin(frame, self.level.map_width(), self.level.map_height());

        for (y, row) in self.level.grid.iter().enumerate() {
            for (x, tile) in row.iter().enumerate() {
                match tile {
                    Tile::Empty => {}
                    _ => frame.render_widget(
                        Span::styled(tile.symbol(), tile.colour()),
                        Rect::new(origin.x + x as u16 * 2, origin.y + y as u16, 2, 1),
                    ),
                }
            }
        }
        frame.render_widget(
            Span::styled(self.level.player.symbol, self.level.player.colour),
            Rect::new(
                origin.x + self.level.player.position.x * 2,
                origin.y + self.level.player.position.y,
                2,
                1,
            ),
        );
        if self.level.won && !self.last_level() {
            self.render_popup(frame, "Well Done!", "\nPress [Enter] to continue...");
        }
        if self.level.won && self.last_level() {
            self.render_popup(frame, "Thanks for Playing!", "\nPress [q] to quit...");
        }
    }

    fn render_popup(&self, frame: &mut Frame, title: &str, msg: &str) {
        let area = frame
            .area()
            .centered(Constraint::Percentage(30), Constraint::Length(5));
        let popup = Paragraph::new(msg).centered().block(
            Block::bordered()
                .title_alignment(Alignment::Center)
                .title(title),
        );
        frame.render_widget(Clear, area);
        frame.render_widget(popup, area);
    }

    /// updates the application's state based on user input
    fn handle_events(&mut self) -> io::Result<()> {
        match event::read()? {
            // it's important to check that the event is a key press event as
            // crossterm also emits key release and repeat events on Windows.
            Event::Key(key_event) if key_event.kind == KeyEventKind::Press => {
                self.handle_key_event(key_event)
            }
            _ => {}
        };
        Ok(())
    }

    fn handle_key_event(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Char('q') => self.exit(),
            KeyCode::Char('c') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                self.exit()
            }
            KeyCode::Up | KeyCode::Char('k') => self.level.try_move_player(Direction::Up),
            KeyCode::Down | KeyCode::Char('j') => self.level.try_move_player(Direction::Down),
            KeyCode::Left | KeyCode::Char('h') => self.level.try_move_player(Direction::Left),
            KeyCode::Right | KeyCode::Char('l') => self.level.try_move_player(Direction::Right),
            KeyCode::Enter if self.level.won => self.next_level(),
            KeyCode::Char('r') => self.restart_level(),
            _ => {}
        }
    }
    fn exit(&mut self) {
        self.exit = true;
    }
}
