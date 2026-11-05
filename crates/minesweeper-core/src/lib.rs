//! Чистая игровая логика классического «Сапёра».
//!
//! В этом крейте нет зависимостей от графики, звука или операционной системы:
//! только поле, мины, флаги, таймер и правила. Благодаря этому логика
//! полностью покрывается обычными `cargo test` без окна и без X11/Wayland.
#![forbid(unsafe_code)]

use std::collections::VecDeque;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

pub const MIN_WIDTH: usize = 10;
pub const MAX_WIDTH: usize = 200;
pub const MIN_HEIGHT: usize = 10;
pub const MAX_HEIGHT: usize = 100;
pub const MAX_MINES: usize = 19_999;
pub const MAX_TIMER_SECONDS: u64 = 999;

/// Готовые уровни. У пресетов число мин фиксировано и не зависит от размера поля.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Preset {
    Beginner,
    Intermediate,
    Expert,
    Custom,
}

impl Preset {
    pub const ALL: [Preset; 4] = [
        Preset::Beginner,
        Preset::Intermediate,
        Preset::Expert,
        Preset::Custom,
    ];

    /// `None` для пользовательского уровня: там число мин задаёт игрок.
    pub const fn fixed_mines(self) -> Option<usize> {
        match self {
            Preset::Beginner => Some(10),
            Preset::Intermediate => Some(40),
            Preset::Expert => Some(99),
            Preset::Custom => None,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Preset::Beginner => "Новичок",
            Preset::Intermediate => "Любитель",
            Preset::Expert => "Профессионал",
            Preset::Custom => "Своя игра",
        }
    }
}

/// Проверенные и ограниченные параметры поля.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Config {
    pub width: usize,
    pub height: usize,
    pub mines: usize,
}

impl Config {
    pub const fn area(self) -> usize {
        self.width * self.height
    }

    /// Максимум мин для заданных размеров: `площадь - 1`, но не больше 19 999.
    pub fn max_mines(width: usize, height: usize) -> usize {
        let width = width.clamp(MIN_WIDTH, MAX_WIDTH);
        let height = height.clamp(MIN_HEIGHT, MAX_HEIGHT);
        (width * height - 1).min(MAX_MINES)
    }

    /// Приводит любые значения к допустимым: 10..=200, 10..=100, 1..=max_mines.
    pub fn new(width: usize, height: usize, mines: usize) -> Self {
        let width = width.clamp(MIN_WIDTH, MAX_WIDTH);
        let height = height.clamp(MIN_HEIGHT, MAX_HEIGHT);
        let mines = mines.clamp(1, Self::max_mines(width, height));
        Self {
            width,
            height,
            mines,
        }
    }

    /// Число мин пресета сохраняется при смене размеров, если оно допустимо.
    pub fn for_preset(preset: Preset, width: usize, height: usize, custom_mines: usize) -> Self {
        let mines = preset.fixed_mines().unwrap_or(custom_mines);
        Self::new(width, height, mines)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Ready,
    Playing,
    Won,
    Lost,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    None,
    Open,
    Flag,
    Boom,
    Win,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cell {
    pub mine: bool,
    pub open: bool,
    pub flag: bool,
    pub around: u8,
}

impl Cell {
    const fn closed() -> Self {
        Self {
            mine: false,
            open: false,
            flag: false,
            around: 0,
        }
    }
}

/// Небольшой xorshift64*: нужен только для раскладки мин.
/// Сид можно передать явно, поэтому тесты детерминированы.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed | 1)
    }

    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, upper: usize) -> usize {
        if upper <= 1 {
            return 0;
        }
        (self.next() % upper as u64) as usize
    }
}

fn time_seed() -> u64 {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    nanos ^ (std::process::id() as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
}

pub struct Game {
    config: Config,
    cells: Vec<Cell>,
    phase: Phase,
    first: bool,
    started: Option<Instant>,
    frozen_secs: f64,
    rng: Rng,
}

impl Game {
    pub fn new(config: Config) -> Self {
        Self::with_seed(config, time_seed())
    }

    pub fn with_seed(config: Config, seed: u64) -> Self {
        let config = Config::new(config.width, config.height, config.mines);
        Self {
            cells: vec![Cell::closed(); config.area()],
            config,
            phase: Phase::Ready,
            first: true,
            started: None,
            frozen_secs: 0.0,
            rng: Rng::new(seed),
        }
    }

    pub fn restart(&mut self) {
        *self = Self::new(self.config);
    }

    pub const fn config(&self) -> Config {
        self.config
    }

    pub const fn width(&self) -> usize {
        self.config.width
    }

    pub const fn height(&self) -> usize {
        self.config.height
    }

    pub const fn mines(&self) -> usize {
        self.config.mines
    }

    pub const fn phase(&self) -> Phase {
        self.phase
    }

    pub fn cells(&self) -> &[Cell] {
        &self.cells
    }

    pub fn flags(&self) -> usize {
        self.cells.iter().filter(|c| c.flag).count()
    }

    /// Счётчик мин на табло: может быть отрицательным, если флагов больше мин.
    pub fn mine_counter(&self) -> i32 {
        self.config.mines as i32 - self.flags() as i32
    }

    pub fn elapsed_secs(&self) -> u64 {
        match self.phase {
            Phase::Ready => 0,
            Phase::Playing => self
                .started
                .map(|t| t.elapsed().as_secs())
                .unwrap_or(0)
                .min(MAX_TIMER_SECONDS),
            Phase::Won | Phase::Lost => (self.frozen_secs as u64).min(MAX_TIMER_SECONDS),
        }
    }

    pub const fn idx(&self, x: usize, y: usize) -> usize {
        y * self.config.width + x
    }

    pub fn cell(&self, x: usize, y: usize) -> Cell {
        self.cells[self.idx(x, y)]
    }

    pub fn neighbors(&self, x: usize, y: usize) -> Vec<(usize, usize)> {
        let mut out = Vec::with_capacity(8);
        let width = self.config.width as isize;
        let height = self.config.height as isize;
        for dy in -1isize..=1 {
            for dx in -1isize..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let nx = x as isize + dx;
                let ny = y as isize + dy;
                if (0..width).contains(&nx) && (0..height).contains(&ny) {
                    out.push((nx as usize, ny as usize));
                }
            }
        }
        out
    }

    /// Открыть клетку. Первый ход всегда безопасен: мины расставляются после него.
    pub fn open(&mut self, x: usize, y: usize) -> Event {
        if self.phase == Phase::Won
            || self.phase == Phase::Lost
            || x >= self.config.width
            || y >= self.config.height
        {
            return Event::None;
        }
        let i = self.idx(x, y);
        if self.cells[i].flag || self.cells[i].open {
            return Event::None;
        }
        if self.first {
            self.plant(x, y);
        }
        if self.cells[i].mine {
            self.cells[i].open = true;
            self.phase = Phase::Lost;
            self.freeze_clock();
            return Event::Boom;
        }
        self.flood(x, y);
        if self.all_safe_open() {
            self.phase = Phase::Won;
            self.freeze_clock();
            self.flag_all_mines();
            return Event::Win;
        }
        Event::Open
    }

    /// Поставить/снять флаг. Таймер classic-игры стартует только с первым открытием.
    pub fn toggle_flag(&mut self, x: usize, y: usize) -> Event {
        if self.phase == Phase::Won
            || self.phase == Phase::Lost
            || x >= self.config.width
            || y >= self.config.height
        {
            return Event::None;
        }
        let i = self.idx(x, y);
        if self.cells[i].open {
            return Event::None;
        }
        self.cells[i].flag = !self.cells[i].flag;
        Event::Flag
    }

    /// Классический «аккорд»: ЛКМ по открытой цифре при нужном числе флагов.
    pub fn chord(&mut self, x: usize, y: usize) -> Event {
        if self.phase != Phase::Playing || x >= self.config.width || y >= self.config.height {
            return Event::None;
        }
        let i = self.idx(x, y);
        if !self.cells[i].open || self.cells[i].around == 0 {
            return Event::None;
        }
        let neighbors = self.neighbors(x, y);
        let flagged = neighbors
            .iter()
            .filter(|&&(nx, ny)| self.cells[self.idx(nx, ny)].flag)
            .count();
        if flagged != self.cells[i].around as usize {
            return Event::None;
        }
        let mut result = Event::Open;
        for (nx, ny) in neighbors {
            let ni = self.idx(nx, ny);
            if self.cells[ni].flag {
                continue;
            }
            match self.open(nx, ny) {
                Event::Boom => return Event::Boom,
                Event::Win => {
                    result = Event::Win;
                    break;
                }
                _ => {}
            }
        }
        result
    }

    /// Расстановка мин после первого хода.
    ///
    /// Резервируется до 9 безопасных клеток (кликнутая + её соседи), но не больше,
    /// чем позволяет плотность: `area - mines`. Оставшиеся клетки — кандидаты под мины;
    /// их всегда достаточно, поэтому нет ни rejection-цикла, ни зависания даже при
    /// `mines == area - 1` (тогда безопасна ровно одна клетка и первый ход сразу победа).
    fn plant(&mut self, x: usize, y: usize) {
        let area = self.cells.len();
        let safe_budget = area - self.config.mines;
        let mut safe = vec![false; area];
        safe[self.idx(x, y)] = true;
        let mut reserved = 1usize;
        for (nx, ny) in self.neighbors(x, y) {
            if reserved < safe_budget {
                safe[self.idx(nx, ny)] = true;
                reserved += 1;
            }
        }
        let mut candidates: Vec<usize> = (0..area).filter(|&i| !safe[i]).collect();
        for i in (1..candidates.len()).rev() {
            let j = self.rng.below(i + 1);
            candidates.swap(i, j);
        }
        for &i in candidates.iter().take(self.config.mines) {
            self.cells[i].mine = true;
        }
        for y in 0..self.config.height {
            for x in 0..self.config.width {
                let i = self.idx(x, y);
                if self.cells[i].mine {
                    continue;
                }
                let count = self
                    .neighbors(x, y)
                    .iter()
                    .filter(|&&(nx, ny)| self.cells[self.idx(nx, ny)].mine)
                    .count();
                self.cells[i].around = count as u8;
            }
        }
        self.first = false;
        self.phase = Phase::Playing;
        self.started = Some(Instant::now());
    }

    fn flood(&mut self, x: usize, y: usize) {
        let mut queue = VecDeque::new();
        let mut queued = vec![false; self.cells.len()];
        queue.push_back((x, y));
        queued[self.idx(x, y)] = true;
        while let Some((cx, cy)) = queue.pop_front() {
            let i = self.idx(cx, cy);
            if self.cells[i].open || self.cells[i].flag || self.cells[i].mine {
                continue;
            }
            self.cells[i].open = true;
            if self.cells[i].around != 0 {
                continue;
            }
            for (nx, ny) in self.neighbors(cx, cy) {
                let ni = self.idx(nx, ny);
                if !queued[ni]
                    && !self.cells[ni].open
                    && !self.cells[ni].flag
                    && !self.cells[ni].mine
                {
                    queued[ni] = true;
                    queue.push_back((nx, ny));
                }
            }
        }
    }

    fn all_safe_open(&self) -> bool {
        self.cells.iter().all(|c| c.mine || c.open)
    }

    fn flag_all_mines(&mut self) {
        for cell in &mut self.cells {
            if cell.mine {
                cell.flag = true;
            }
        }
    }

    fn freeze_clock(&mut self) {
        if let Some(start) = self.started.take() {
            self.frozen_secs = start.elapsed().as_secs_f64();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn find_mine(game: &Game) -> (usize, usize) {
        for y in 0..game.height() {
            for x in 0..game.width() {
                if game.cell(x, y).mine {
                    return (x, y);
                }
            }
        }
        panic!("на поле нет мин");
    }

    #[test]
    fn config_limits_width_height_and_mines() {
        assert_eq!(
            Config::new(0, 0, 0),
            Config {
                width: 10,
                height: 10,
                mines: 1
            }
        );
        assert_eq!(Config::new(5, 5, 0).width, MIN_WIDTH);
        assert_eq!(Config::new(5, 5, 0).height, MIN_HEIGHT);
        assert_eq!(Config::new(5, 5, 0).mines, 1);
        assert_eq!(Config::new(999, 999, 999).width, MAX_WIDTH);
        assert_eq!(Config::new(999, 999, 999).height, MAX_HEIGHT);
        assert_eq!(Config::max_mines(MAX_WIDTH, MAX_HEIGHT), MAX_MINES);
        assert_eq!(
            Config::new(MAX_WIDTH, MAX_HEIGHT, usize::MAX).mines,
            MAX_MINES
        );
        assert_eq!(Config::new(10, 10, 100).mines, 99);
        assert_eq!(Config::new(200, 100, 20_000).mines, 19_999);
        assert_eq!(Config::new(30, 16, 479).mines, 479);
        assert_eq!(Config::new(30, 16, 480).mines, 479);
        for width in [10, 11, 50, 200] {
            for height in [10, 17, 100] {
                let config = Config::new(width, height, 1);
                assert!(config.mines >= 1);
                assert!(config.mines < config.area());
            }
        }
    }

    #[test]
    fn presets_keep_fixed_mine_count_when_size_changes() {
        let expected = [
            (Preset::Beginner, 10),
            (Preset::Intermediate, 40),
            (Preset::Expert, 99),
        ];
        for (preset, mines) in expected {
            assert_eq!(preset.fixed_mines(), Some(mines));
            for (width, height) in [(10, 10), (10, 100), (200, 10), (30, 16), (200, 100)] {
                let config = Config::for_preset(preset, width, height, 777);
                assert_eq!(config.mines, mines, "{preset:?} {width}x{height}");
                assert_eq!(Game::new(config).mines(), mines);
            }
        }
        assert_eq!(Preset::Custom.fixed_mines(), None);
        assert_eq!(Config::for_preset(Preset::Custom, 20, 20, 55).mines, 55);
    }

    #[test]
    fn first_click_is_safe_for_many_seeds_and_positions() {
        for seed in 0..32u64 {
            let game_config = Config::new(30, 16, 99);
            let (x, y) = ((seed as usize * 7) % 30, (seed as usize * 11) % 16);
            let mut game = Game::with_seed(game_config, seed);
            let event = game.open(x, y);
            assert_ne!(event, Event::Boom, "seed {seed}");
            assert_ne!(game.phase(), Phase::Lost, "seed {seed}");
            assert!(!game.cell(x, y).mine, "seed {seed}");
            assert!(game.cell(x, y).open, "seed {seed}");
            assert_eq!(game.cells().iter().filter(|c| c.mine).count(), 99);
        }
    }

    #[test]
    fn max_density_never_hangs_and_wins_immediately() {
        let cases = [
            (10, 10, 99),
            (10, 100, 999),
            (50, 50, 2499),
            (200, 100, 19_999),
        ];
        for (width, height, mines) in cases {
            let config = Config::new(width, height, mines);
            assert_eq!(config.mines, mines);
            for seed in 0..4u64 {
                let mut game = Game::with_seed(config, seed + 1);
                let event = game.open(width / 2, height / 2);
                assert_eq!(event, Event::Win, "{width}x{height} seed {seed}");
                assert_eq!(game.phase(), Phase::Won);
                assert_eq!(game.cells().iter().filter(|c| c.mine).count(), mines);
                assert!(game.cells().iter().all(|c| c.mine || c.open));
                assert_eq!(game.flags(), mines);
                assert_eq!(game.mine_counter(), 0);
            }
        }
    }

    #[test]
    fn first_click_opens_empty_area_and_numbers_are_correct() {
        let config = Config::new(10, 10, 10);
        let mut game = Game::with_seed(config, 42);
        let event = game.open(4, 4);
        assert_eq!(event, Event::Open);
        assert_eq!(game.phase(), Phase::Playing);
        let opened = game.cells().iter().filter(|c| c.open).count();
        assert!(
            opened >= 9,
            "должна открыться область, а не одна клетка: {opened}"
        );
        assert!(!game.cell(4, 4).mine);
        assert_eq!(game.cell(4, 4).around, 0);
        for y in 0..game.height() {
            for x in 0..game.width() {
                let cell = game.cell(x, y);
                if cell.mine {
                    continue;
                }
                let expected = game
                    .neighbors(x, y)
                    .iter()
                    .filter(|&&(nx, ny)| game.cell(nx, ny).mine)
                    .count() as u8;
                assert_eq!(cell.around, expected, "цифра в ({x}, {y})");
            }
        }
    }

    #[test]
    fn flag_can_be_set_and_removed() {
        let config = Config::new(10, 10, 10);
        let mut game = Game::with_seed(config, 3);
        assert_eq!(game.flags(), 0);
        assert_eq!(game.mine_counter(), 10);
        assert_eq!(game.toggle_flag(2, 2), Event::Flag);
        assert!(game.cell(2, 2).flag);
        assert_eq!(game.flags(), 1);
        assert_eq!(game.mine_counter(), 9);
        assert_eq!(game.phase(), Phase::Ready);
        assert_eq!(game.elapsed_secs(), 0);
        assert_eq!(game.toggle_flag(2, 2), Event::Flag);
        assert!(!game.cell(2, 2).flag);
        assert_eq!(game.flags(), 0);
        assert_eq!(game.toggle_flag(999, 999), Event::None);
        game.open(5, 5);
        assert_eq!(
            game.toggle_flag(5, 5),
            Event::None,
            "открытую клетку нельзя флаговать"
        );
    }

    #[test]
    fn opening_all_safe_cells_wins() {
        let config = Config::new(12, 10, 14);
        let mut game = Game::with_seed(config, 7);
        game.open(0, 0);
        let safe: Vec<usize> = (0..game.cells().len())
            .filter(|&i| !game.cells()[i].mine && !game.cells()[i].open)
            .collect();
        for i in safe {
            let x = i % game.width();
            let y = i / game.width();
            game.open(x, y);
        }
        assert_eq!(game.phase(), Phase::Won);
        assert_eq!(game.mine_counter(), 0);
        assert_eq!(
            game.open(0, 0),
            Event::None,
            "после победы ввод игнорируется"
        );
    }

    #[test]
    fn opening_a_mine_loses_and_reveals_it() {
        let config = Config::new(10, 10, 10);
        let mut game = Game::with_seed(config, 11);
        game.open(4, 4);
        let (mx, my) = find_mine(&game);
        assert_eq!(game.toggle_flag(mx, my), Event::Flag);
        let (ox, oy) = (0..game.cells().len())
            .map(|i| (i % game.width(), i / game.width()))
            .find(|&(x, y)| game.cell(x, y).mine && !game.cell(x, y).flag)
            .expect("должна найтись незафлагованная мина");
        assert_eq!(game.open(ox, oy), Event::Boom);
        assert_eq!(game.phase(), Phase::Lost);
        assert!(game.cell(ox, oy).open);
        assert_eq!(game.open(0, 0), Event::None);
        assert_eq!(game.toggle_flag(0, 0), Event::None);
    }

    #[test]
    fn chord_opens_neighbours_when_flags_are_correct() {
        let config = Config::new(12, 12, 18);
        let mut game = Game::with_seed(config, 99);
        game.open(6, 6);
        let target = (0..game.cells().len())
            .map(|i| (i % game.width(), i / game.width()))
            .find(|&(x, y)| {
                let cell = game.cell(x, y);
                cell.open
                    && cell.around > 0
                    && game
                        .neighbors(x, y)
                        .iter()
                        .any(|&(nx, ny)| !game.cell(nx, ny).open)
            })
            .expect("нужна открытая цифра с закрытыми соседями");
        let (x, y) = target;
        let closed_before = game
            .neighbors(x, y)
            .iter()
            .filter(|&&(nx, ny)| !game.cell(nx, ny).open && !game.cell(nx, ny).flag)
            .count();
        assert!(closed_before > 0);
        assert_eq!(
            game.chord(x, y),
            Event::None,
            "без флагов аккорд не срабатывает"
        );
        for (nx, ny) in game.neighbors(x, y) {
            if game.cell(nx, ny).mine && !game.cell(nx, ny).open {
                game.toggle_flag(nx, ny);
            }
        }
        let event = game.chord(x, y);
        assert_ne!(event, Event::Boom, "верные флаги не должны взрывать мину");
        let closed_after = game
            .neighbors(x, y)
            .iter()
            .filter(|&&(nx, ny)| !game.cell(nx, ny).open && !game.cell(nx, ny).flag)
            .count();
        assert!(
            closed_after < closed_before,
            "аккорд должен открыть соседей"
        );
    }

    #[test]
    fn restart_resets_everything() {
        let config = Config::new(10, 10, 10);
        let mut game = Game::with_seed(config, 5);
        game.toggle_flag(1, 1);
        game.open(5, 5);
        game.restart();
        assert_eq!(game.phase(), Phase::Ready);
        assert_eq!(game.flags(), 0);
        assert_eq!(game.elapsed_secs(), 0);
        assert!(game.cells().iter().all(|c| !c.open && !c.mine && !c.flag));
    }
}
