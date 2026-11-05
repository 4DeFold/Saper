//! Классический «Сапёр»: окно, отрисовка и ввод на macroquad.
//!
//! Вся игровая логика вынесена в `minesweeper-core` и покрыта тестами.
//! Здесь только представление: серые выпуклые плитки, цифры, флаги, счётчик,
//! таймер, лицо-перезапуск, настройки, масштабирование и звуки.
#![forbid(unsafe_code)]

use macroquad::{
    audio::{load_sound_from_bytes, play_sound, PlaySoundParams, Sound},
    prelude::*,
};
use minesweeper_core::{
    Config, Event, Game, Phase, Preset, MAX_HEIGHT, MAX_MINES, MAX_WIDTH, MIN_HEIGHT, MIN_WIDTH,
};

const FACE: Color = Color::new(0.78, 0.78, 0.78, 1.0);
const BOARD_BG: Color = Color::new(0.56, 0.56, 0.56, 1.0);
const OPEN_BG: Color = Color::new(0.73, 0.73, 0.73, 1.0);
const CELL_MIN: f32 = 14.0;
const CELL_MAX: f32 = 44.0;
const FONT_BYTES: &[u8] = include_bytes!("../assets/DejaVuSans.ttf");

#[derive(Clone, Copy, PartialEq, Eq)]
enum Edit {
    Width,
    Height,
    Mines,
    KeyFlag,
    KeyOpen,
}

struct App {
    game: Game,
    preset: Preset,
    width: usize,
    height: usize,
    custom_mines: usize,
    settings: bool,
    edit: Option<Edit>,
    edit_buffer: String,
    key_flag: KeyCode,
    key_open: KeyCode,
    zoom: f32,
    cam: Vec2,
    dragging: bool,
    drag_at: Vec2,
    pan_at: Vec2,
    fullscreen: bool,
    sound_on: bool,
    smile_down: bool,
    font: Font,
    sounds: [Sound; 3],
}

impl App {
    fn new(font: Font, sounds: [Sound; 3]) -> Self {
        let (width, height) = preset_size(Preset::Beginner);
        let config = Config::for_preset(Preset::Beginner, width, height, 10);
        Self {
            game: Game::new(config),
            preset: Preset::Beginner,
            width,
            height,
            custom_mines: 10,
            settings: false,
            edit: None,
            edit_buffer: String::new(),
            key_flag: KeyCode::Q,
            key_open: KeyCode::E,
            zoom: 1.0,
            cam: Vec2::ZERO,
            dragging: false,
            drag_at: Vec2::ZERO,
            pan_at: Vec2::ZERO,
            fullscreen: false,
            sound_on: true,
            smile_down: false,
            font,
            sounds,
        }
    }

    fn play(&self, index: usize) {
        if self.sound_on {
            play_sound(
                &self.sounds[index],
                PlaySoundParams {
                    looped: false,
                    volume: 0.55,
                },
            );
        }
    }

    fn apply_board(&mut self) {
        let config = Config::for_preset(self.preset, self.width, self.height, self.custom_mines);
        self.game = Game::new(config);
        self.cam = Vec2::ZERO;
    }

    fn close_edit(&mut self) {
        self.edit = None;
        self.edit_buffer.clear();
    }
}

#[derive(Clone, Copy)]
struct Layout {
    panel_x: f32,
    panel_w: f32,
    header_y: f32,
    toolbar_y: f32,
    toolbar_h: f32,
    board_x: f32,
    board_y: f32,
    board_w: f32,
    board_h: f32,
    status_y: f32,
    cell: f32,
}

struct Dialog {
    x: f32,
    y: f32,
    s: f32,
}

impl Dialog {
    const W: f32 = 520.0;
    const H: f32 = 540.0;

    fn new(sw: f32, sh: f32) -> Self {
        let s = ((sw - 32.0).min(Self::W) / Self::W).min((sh - 32.0).min(Self::H) / Self::H);
        let s = s.max(0.35);
        Self {
            x: (sw - Self::W * s) / 2.0,
            y: (sh - Self::H * s) / 2.0,
            s,
        }
    }

    fn x(&self, v: f32) -> f32 {
        self.x + v * self.s
    }

    fn y(&self, v: f32) -> f32 {
        self.y + v * self.s
    }

    fn w(&self, v: f32) -> f32 {
        v * self.s
    }

    fn fs(&self, v: f32) -> f32 {
        (v * self.s).max(8.0)
    }
}

struct Smoke {
    frames: u32,
    marker: Option<String>,
}

fn smoke_options() -> Option<Smoke> {
    let mut frames = None;
    let mut marker = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--smoke" => frames = args.next().and_then(|v| v.parse::<u32>().ok()),
            "--marker" => marker = args.next(),
            _ => {}
        }
    }
    frames.map(|frames| Smoke { frames, marker })
}

#[macroquad::main(window_conf)]
async fn main() {
    let font = load_ttf_font_from_bytes(FONT_BYTES)
        .expect("не удалось загрузить встроенный шрифт DejaVu Sans");
    let sounds = [
        load_sound_from_bytes(&wav(&[(580.0, 0.055), (820.0, 0.045)], 0.14))
            .await
            .expect("не удалось создать звук открытия"),
        load_sound_from_bytes(&wav(&[(260.0, 0.08), (160.0, 0.13)], 0.20))
            .await
            .expect("не удалось создать звук проигрыша"),
        load_sound_from_bytes(&wav(&[(660.0, 0.05), (880.0, 0.05), (1040.0, 0.11)], 0.17))
            .await
            .expect("не удалось создать звук победы"),
    ];
    let mut app = App::new(font, sounds);
    let smoke = smoke_options();
    let mut drawn = 0u32;
    loop {
        clear_background(Color::new(0.72, 0.72, 0.72, 1.0));
        frame(&mut app);
        if let Some(smoke) = &smoke {
            drawn += 1;
            if drawn >= smoke.frames {
                let message = format!("SMOKE OK: frames={drawn}\n");
                if let Some(path) = &smoke.marker {
                    let _ = std::fs::write(path, message.as_bytes());
                }
                println!("{message}");
                std::process::exit(0);
            }
        }
        next_frame().await;
    }
}

fn window_conf() -> Conf {
    Conf {
        window_title: "Сапёр — классический".to_owned(),
        window_width: 1120,
        window_height: 780,
        window_resizable: true,
        high_dpi: true,
        ..Default::default()
    }
}

/// Короткие PCM WAV-сигналы генерируются на лету, без внешних файлов.
fn wav(notes: &[(f32, f32)], volume: f32) -> Vec<u8> {
    const RATE: u32 = 22_050;
    let mut samples: Vec<i16> = Vec::new();
    for &(freq, duration) in notes {
        let count = (RATE as f32 * duration) as usize;
        for i in 0..count {
            let t = i as f32 / RATE as f32;
            let envelope = (1.0 - i as f32 / count as f32).max(0.0);
            let value = (t * freq * std::f32::consts::TAU).sin() * volume * envelope;
            samples.push((value * i16::MAX as f32) as i16);
        }
    }
    let data_len = (samples.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&RATE.to_le_bytes());
    out.extend_from_slice(&(RATE * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for sample in samples {
        out.extend_from_slice(&sample.to_le_bytes());
    }
    out
}

fn frame(app: &mut App) {
    let sw = screen_width();
    let sh = screen_height();
    let (mx, my) = mouse_position();
    let layout = compute_layout(app, sw, sh);

    draw_panel(sw, sh, layout.panel_x, layout.panel_w);
    draw_header(app, &layout);
    draw_toolbar(app, &layout);
    draw_board(app, &layout);
    draw_status(app, &layout);

    if app.settings {
        draw_settings(app, sw, sh, mx, my);
    } else {
        handle_header_click(app, &layout, mx, my);
        handle_toolbar_click(app, &layout, mx, my);
        handle_board_input(app, &layout, mx, my);
    }

    if is_key_pressed(KeyCode::F11) {
        app.fullscreen = !app.fullscreen;
        set_fullscreen(app.fullscreen);
    }
    if is_key_pressed(KeyCode::Escape) && app.settings {
        app.settings = false;
        app.close_edit();
    }
    if is_mouse_button_released(MouseButton::Left) || is_mouse_button_released(MouseButton::Middle)
    {
        app.smile_down = false;
        app.dragging = false;
    }

    let updated = compute_layout(app, sw, sh);
    clamp_cam(app, &updated);
}

fn compute_layout(app: &App, sw: f32, sh: f32) -> Layout {
    let margin = 16.0;
    let panel_w = (sw - 2.0 * margin).clamp(160.0, 980.0).min(sw);
    let panel_x = (sw - panel_w) / 2.0;
    let cell = (28.0 * app.zoom).clamp(CELL_MIN, CELL_MAX);
    let header_y = 18.0;
    let toolbar_y = 96.0;
    let toolbar_h = 30.0;
    let board_y = toolbar_y + toolbar_h + 22.0;
    let board_h = (sh - board_y - 52.0).max(60.0);
    let board_w = (panel_w - 40.0)
        .min(app.game.width() as f32 * cell)
        .max(60.0)
        .min(sw - 16.0);
    let board_x = (sw - board_w) / 2.0;
    let status_y = (board_y + board_h + 26.0).min(sh - 12.0);
    Layout {
        panel_x,
        panel_w,
        header_y,
        toolbar_y,
        toolbar_h,
        board_x,
        board_y,
        board_w,
        board_h,
        status_y,
        cell,
    }
}

fn clamp_cam(app: &mut App, l: &Layout) {
    let max_x = (app.game.width() as f32 * l.cell - l.board_w).max(0.0);
    let max_y = (app.game.height() as f32 * l.cell - l.board_h).max(0.0);
    app.cam.x = app.cam.x.clamp(0.0, max_x);
    app.cam.y = app.cam.y.clamp(0.0, max_y);
}

fn hit(mx: f32, my: f32, x: f32, y: f32, w: f32, h: f32) -> bool {
    (x..=x + w).contains(&mx) && (y..=y + h).contains(&my)
}

fn text(font: &Font, s: &str, x: f32, y: f32, size: f32, color: Color) {
    draw_text_ex(
        s,
        x,
        y,
        TextParams {
            font: Some(font),
            font_size: (size.max(8.0)) as u16,
            color,
            ..Default::default()
        },
    );
}

fn centered(font: &Font, s: &str, x: f32, y: f32, size: f32, color: Color) {
    let dims = measure_text(s, Some(font), (size.max(8.0)) as u16, 1.0);
    text(font, s, x - dims.width / 2.0, y, size, color);
}

fn bevel(x: f32, y: f32, w: f32, h: f32, pressed: bool) {
    draw_rectangle(x, y, w, h, FACE);
    if pressed {
        draw_line(x, y, x + w, y, 2.0, GRAY);
        draw_line(x, y, x, y + h, 2.0, GRAY);
        draw_line(x, y + h, x + w, y + h, 2.0, DARKGRAY);
        draw_line(x + w, y, x + w, y + h, 2.0, DARKGRAY);
    } else {
        draw_line(x, y, x + w, y, 3.0, WHITE);
        draw_line(x, y, x, y + h, 3.0, WHITE);
        draw_line(
            x,
            y + h,
            x + w,
            y + h,
            3.0,
            Color::new(0.35, 0.35, 0.35, 1.0),
        );
        draw_line(
            x + w,
            y,
            x + w,
            y + h,
            3.0,
            Color::new(0.35, 0.35, 0.35, 1.0),
        );
        draw_line(
            x + 2.0,
            y + h - 2.0,
            x + w - 2.0,
            y + h - 2.0,
            1.0,
            Color::new(0.54, 0.54, 0.54, 1.0),
        );
    }
}

fn inset(x: f32, y: f32, w: f32, h: f32) {
    draw_rectangle(x, y, w, h, Color::new(0.47, 0.47, 0.47, 1.0));
    draw_line(x, y, x + w, y, 2.0, Color::new(0.28, 0.28, 0.28, 1.0));
    draw_line(x, y, x, y + h, 2.0, Color::new(0.28, 0.28, 0.28, 1.0));
    draw_line(x, y + h, x + w, y + h, 2.0, WHITE);
    draw_line(x + w, y, x + w, y + h, 2.0, WHITE);
}

fn polyline(points: &[(f32, f32)], thickness: f32, color: Color) {
    for pair in points.windows(2) {
        draw_line(pair[0].0, pair[0].1, pair[1].0, pair[1].1, thickness, color);
    }
}

fn mine_icon(x: f32, y: f32, size: f32) {
    let r = size * 0.23;
    draw_circle(x, y, r, BLACK);
    for a in 0..8 {
        let t = a as f32 * std::f32::consts::PI / 4.0;
        draw_line(
            x + t.cos() * r * 0.75,
            y + t.sin() * r * 0.75,
            x + t.cos() * r * 1.65,
            y + t.sin() * r * 1.65,
            size * 0.075,
            BLACK,
        );
    }
    draw_circle(x - r * 0.28, y - r * 0.30, r * 0.18, WHITE);
}

fn flag_icon(x: f32, y: f32, size: f32) {
    let u = size / 24.0;
    draw_line(
        x + 5.0 * u,
        y + 21.0 * u,
        x + 18.0 * u,
        y + 21.0 * u,
        2.0 * u,
        BLACK,
    );
    draw_line(
        x + 11.0 * u,
        y + 5.0 * u,
        x + 11.0 * u,
        y + 20.0 * u,
        2.0 * u,
        BLACK,
    );
    draw_triangle(
        vec2(x + 11.0 * u, y + 5.0 * u),
        vec2(x + 11.0 * u, y + 15.0 * u),
        vec2(x + 20.0 * u, y + 9.0 * u),
        RED,
    );
}

fn number_color(n: u8) -> Color {
    match n {
        1 => BLUE,
        2 => GREEN,
        3 => RED,
        4 => Color::new(0.05, 0.05, 0.55, 1.0),
        5 => Color::new(0.55, 0.0, 0.0, 1.0),
        6 => Color::new(0.0, 0.45, 0.45, 1.0),
        _ => BLACK,
    }
}

fn draw_digit(font: &Font, x: f32, y: f32, size: f32, value: i32) {
    let shown = if value < 0 {
        let v = (-value).min(99);
        format!("-{v:02}")
    } else {
        let v = value.min(999);
        format!("{v:03}")
    };
    for (i, ch) in shown.chars().enumerate() {
        let cx = x + i as f32 * size * 0.62;
        draw_rectangle(
            cx,
            y,
            size * 0.58,
            size * 1.15,
            Color::new(0.22, 0.22, 0.22, 1.0),
        );
        let color = if ch == '-' {
            RED
        } else {
            Color::new(1.0, 0.12, 0.12, 1.0)
        };
        text(
            font,
            &ch.to_string(),
            cx + 1.0,
            y + size * 0.93,
            size * 1.02,
            color,
        );
    }
}

fn draw_face(x: f32, y: f32, size: f32, phase: Phase) {
    let cx = x + size / 2.0;
    let cy = y + size / 2.0;
    let r = size * 0.36;
    draw_circle(cx, cy, r + 1.5, BLACK);
    draw_circle(cx, cy, r, YELLOW);
    draw_circle(cx - r * 0.36, cy - r * 0.24, r * 0.12, BLACK);
    draw_circle(cx + r * 0.36, cy - r * 0.24, r * 0.12, BLACK);
    let mouth = match phase {
        Phase::Lost => vec![
            (cx - r * 0.45, cy + r * 0.55),
            (cx, cy + r * 0.15),
            (cx + r * 0.45, cy + r * 0.55),
        ],
        Phase::Won => vec![
            (cx - r * 0.60, cy + r * 0.15),
            (cx, cy + r * 0.70),
            (cx + r * 0.60, cy + r * 0.15),
        ],
        _ => vec![
            (cx - r * 0.50, cy + r * 0.22),
            (cx, cy + r * 0.55),
            (cx + r * 0.50, cy + r * 0.22),
        ],
    };
    polyline(&mouth, (size * 0.055).max(1.0), BLACK);
    if phase == Phase::Won {
        draw_rectangle(cx - r * 0.70, cy - r * 0.42, r * 1.40, r * 0.24, BLACK);
    }
}

fn draw_panel(sw: f32, sh: f32, px: f32, pw: f32) {
    let _ = sw;
    draw_rectangle(px, 10.0, pw, sh - 20.0, FACE);
    draw_line(px, 10.0, px + pw, 10.0, 3.0, WHITE);
    draw_line(px, 10.0, px, sh - 10.0, 3.0, WHITE);
    draw_line(
        px,
        sh - 10.0,
        px + pw,
        sh - 10.0,
        3.0,
        Color::new(0.3, 0.3, 0.3, 1.0),
    );
    draw_line(
        px + pw,
        10.0,
        px + pw,
        sh - 10.0,
        3.0,
        Color::new(0.3, 0.3, 0.3, 1.0),
    );
}

fn preset_label(preset: Preset) -> &'static str {
    match preset {
        Preset::Beginner => "НОВИЧОК",
        Preset::Intermediate => "ЛЮБИТЕЛЬ",
        Preset::Expert => "ПРОФЕССИОНАЛ",
        Preset::Custom => "СВОЯ ИГРА",
    }
}

fn preset_size(preset: Preset) -> (usize, usize) {
    match preset {
        Preset::Beginner => (10, 10),
        Preset::Intermediate => (16, 16),
        Preset::Expert => (30, 16),
        Preset::Custom => (10, 10),
    }
}

fn draw_header(app: &App, l: &Layout) {
    let font = &app.font;
    let size = 22.0;
    let digits_w = size * 0.62 * 3.0;
    draw_digit(
        font,
        l.panel_x + 18.0,
        l.header_y + 6.0,
        size,
        app.game.mine_counter(),
    );
    draw_digit(
        font,
        l.panel_x + l.panel_w - 18.0 - digits_w,
        l.header_y + 6.0,
        size,
        app.game.elapsed_secs() as i32,
    );
    let face_size = 40.0;
    let fx = l.panel_x + l.panel_w / 2.0 - face_size / 2.0;
    let fy = l.header_y + 4.0;
    bevel(fx, fy, face_size, face_size, app.smile_down);
    draw_face(fx + 3.0, fy + 3.0, face_size - 6.0, app.game.phase());

    mine_icon(l.panel_x + 30.0, l.header_y + 52.0, 26.0);
    text(
        font,
        "САПЁР",
        l.panel_x + 46.0,
        l.header_y + 58.0,
        15.0,
        BLACK,
    );
    text(
        font,
        "CLASSIC",
        l.panel_x + 46.0,
        l.header_y + 70.0,
        9.0,
        Color::new(0.25, 0.25, 0.25, 1.0),
    );
    centered(
        font,
        preset_label(app.preset),
        l.panel_x + l.panel_w / 2.0,
        l.header_y + 68.0,
        12.0,
        Color::new(0.16, 0.16, 0.16, 1.0),
    );
    text(
        font,
        "МИНЫ",
        l.panel_x + 18.0,
        l.header_y + 45.0,
        9.0,
        Color::new(0.2, 0.2, 0.2, 1.0),
    );
    centered(
        font,
        "ВРЕМЯ",
        l.panel_x + l.panel_w - 18.0 - digits_w / 2.0,
        l.header_y + 45.0,
        9.0,
        Color::new(0.2, 0.2, 0.2, 1.0),
    );
}

fn handle_header_click(app: &mut App, l: &Layout, mx: f32, my: f32) {
    let face_size = 40.0;
    let fx = l.panel_x + l.panel_w / 2.0 - face_size / 2.0;
    let fy = l.header_y + 4.0;
    if is_mouse_button_pressed(MouseButton::Left) && hit(mx, my, fx, fy, face_size, face_size) {
        app.game.restart();
        app.cam = Vec2::ZERO;
        app.smile_down = true;
    }
}

fn toolbar_items(app: &App) -> [String; 6] {
    let pct = (app.zoom * 100.0) as i32;
    [
        "Настройки".to_owned(),
        "−".to_owned(),
        format!("{pct}%"),
        "+".to_owned(),
        if app.sound_on {
            "Звук: вкл".to_owned()
        } else {
            "Звук: выкл".to_owned()
        },
        "Полный экран".to_owned(),
    ]
}

fn toolbar_geometry(l: &Layout) -> (f32, f32, f32, [f32; 6]) {
    let widths = [90.0, 30.0, 52.0, 30.0, 92.0, 112.0];
    let gap = 7.0;
    let total: f32 = widths.iter().sum::<f32>() + gap * 5.0;
    let scale = ((l.panel_w - 24.0) / total).clamp(0.5, 1.0);
    let scaled: [f32; 6] = std::array::from_fn(|i| widths[i] * scale);
    let total_scaled: f32 = scaled.iter().sum::<f32>() + gap * scale * 5.0;
    let start = l.panel_x + (l.panel_w - total_scaled) / 2.0;
    (start, gap * scale, l.toolbar_h, scaled)
}

fn draw_toolbar(app: &App, l: &Layout) {
    let (mut x, gap, h, widths) = toolbar_geometry(l);
    let labels = toolbar_items(app);
    for (label, w) in labels.iter().zip(widths.iter()) {
        bevel(x, l.toolbar_y, *w, h, false);
        centered(
            &app.font,
            label,
            x + w / 2.0,
            l.toolbar_y + 19.0,
            12.0,
            BLACK,
        );
        x += w + gap;
    }
}

fn handle_toolbar_click(app: &mut App, l: &Layout, mx: f32, my: f32) {
    let (mut x, gap, h, widths) = toolbar_geometry(l);
    for (i, w) in widths.iter().enumerate() {
        if is_mouse_button_pressed(MouseButton::Left) && hit(mx, my, x, l.toolbar_y, *w, h) {
            match i {
                0 => {
                    app.settings = true;
                    app.close_edit();
                }
                1 => app.zoom = (app.zoom - 0.1).clamp(0.5, 1.6),
                2 => app.zoom = 1.0,
                3 => app.zoom = (app.zoom + 0.1).clamp(0.5, 1.6),
                4 => app.sound_on = !app.sound_on,
                5 => {
                    app.fullscreen = !app.fullscreen;
                    set_fullscreen(app.fullscreen);
                }
                _ => {}
            }
        }
        x += w + gap;
    }
}

fn draw_board(app: &App, l: &Layout) {
    inset(
        l.board_x - 7.0,
        l.board_y - 7.0,
        l.board_w + 14.0,
        l.board_h + 14.0,
    );
    draw_rectangle(l.board_x, l.board_y, l.board_w, l.board_h, BOARD_BG);
    let right = l.board_x + l.board_w;
    let bottom = l.board_y + l.board_h;
    let first_x = (app.cam.x / l.cell).floor() as usize;
    let first_y = (app.cam.y / l.cell).floor() as usize;
    let last_x = (((app.cam.x + l.board_w) / l.cell).ceil() as usize + 1).min(app.game.width());
    let last_y = (((app.cam.y + l.board_h) / l.cell).ceil() as usize + 1).min(app.game.height());
    for y in first_y..last_y {
        for x in first_x..last_x {
            let sx = l.board_x + x as f32 * l.cell - app.cam.x;
            let sy = l.board_y + y as f32 * l.cell - app.cam.y;
            let cx0 = sx.max(l.board_x);
            let cy0 = sy.max(l.board_y);
            let cx1 = (sx + l.cell).min(right);
            let cy1 = (sy + l.cell).min(bottom);
            if cx1 <= cx0 || cy1 <= cy0 {
                continue;
            }
            let dw = cx1 - cx0;
            let dh = cy1 - cy0;
            let full = dw >= l.cell - 0.5 && dh >= l.cell - 0.5;
            let cell = app.game.cell(x, y);
            if cell.open {
                draw_rectangle(cx0, cy0, dw, dh, OPEN_BG);
                draw_line(
                    cx0,
                    cy0 + dh,
                    cx1,
                    cy0 + dh,
                    1.0,
                    Color::new(0.46, 0.46, 0.46, 1.0),
                );
                draw_line(cx1, cy0, cx1, cy1, 1.0, Color::new(0.46, 0.46, 0.46, 1.0));
            } else {
                bevel(cx0, cy0, dw, dh, false);
            }
            if !full {
                continue;
            }
            if cell.open {
                if cell.mine {
                    mine_icon(sx + l.cell / 2.0, sy + l.cell / 2.0, l.cell * 0.76);
                } else if cell.around > 0 {
                    centered(
                        &app.font,
                        &cell.around.to_string(),
                        sx + l.cell / 2.0,
                        sy + l.cell * 0.79,
                        l.cell * 0.77,
                        number_color(cell.around),
                    );
                }
            } else if cell.flag {
                flag_icon(sx + l.cell * 0.12, sy + l.cell * 0.10, l.cell * 0.78);
                if app.game.phase() == Phase::Lost && !cell.mine {
                    draw_line(
                        sx + 3.0,
                        sy + 3.0,
                        sx + l.cell - 3.0,
                        sy + l.cell - 3.0,
                        2.0,
                        RED,
                    );
                    draw_line(
                        sx + l.cell - 3.0,
                        sy + 3.0,
                        sx + 3.0,
                        sy + l.cell - 3.0,
                        2.0,
                        RED,
                    );
                }
            } else if app.game.phase() == Phase::Lost && cell.mine {
                mine_icon(sx + l.cell / 2.0, sy + l.cell / 2.0, l.cell * 0.76);
            } else if app.game.phase() == Phase::Won && cell.mine {
                flag_icon(sx + l.cell * 0.12, sy + l.cell * 0.10, l.cell * 0.78);
            }
        }
    }
}

fn draw_status(app: &App, l: &Layout) {
    let s = match app.game.phase() {
        Phase::Ready => "Первый ход безопасен — откроется максимально возможная область",
        Phase::Playing => {
            "ЛКМ — открыть  ·  ПКМ / Q — флаг  ·  E — открыть  ·  колёсико — прокрутка  ·  Shift+перетаскивание — сдвиг"
        }
        Phase::Won => "Победа! Нажмите на лицо, чтобы сыграть ещё раз.",
        Phase::Lost => "Игра окончена. Нажмите на лицо, чтобы начать заново.",
    };
    let color = if app.game.phase() == Phase::Lost {
        Color::new(0.45, 0.05, 0.05, 1.0)
    } else {
        Color::new(0.2, 0.2, 0.2, 1.0)
    };
    centered(&app.font, s, screen_width() / 2.0, l.status_y, 12.0, color);
}

fn cell_at(app: &App, l: &Layout, mx: f32, my: f32) -> Option<(usize, usize)> {
    if !hit(mx, my, l.board_x, l.board_y, l.board_w, l.board_h) {
        return None;
    }
    let x = ((mx - l.board_x + app.cam.x) / l.cell) as usize;
    let y = ((my - l.board_y + app.cam.y) / l.cell) as usize;
    if x < app.game.width() && y < app.game.height() {
        Some((x, y))
    } else {
        None
    }
}

fn event_sound(app: &App, event: Event) {
    match event {
        Event::Boom => app.play(1),
        Event::Win => app.play(2),
        Event::Open | Event::Flag => app.play(0),
        Event::None => {}
    }
}

fn handle_board_input(app: &mut App, l: &Layout, mx: f32, my: f32) {
    let point = vec2(mx, my);
    let inside = hit(mx, my, l.board_x, l.board_y, l.board_w, l.board_h);
    let shift = is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift);
    let ctrl = is_key_down(KeyCode::LeftControl)
        || is_key_down(KeyCode::RightControl)
        || is_key_down(KeyCode::LeftSuper)
        || is_key_down(KeyCode::RightSuper);

    if is_mouse_button_pressed(MouseButton::Left) && inside && shift {
        app.dragging = true;
        app.drag_at = point;
        app.pan_at = app.cam;
    }
    if is_mouse_button_pressed(MouseButton::Middle) && inside {
        app.dragging = true;
        app.drag_at = point;
        app.pan_at = app.cam;
    }
    if app.dragging
        && (is_mouse_button_down(MouseButton::Left) || is_mouse_button_down(MouseButton::Middle))
    {
        app.cam = app.pan_at - (point - app.drag_at);
    }

    let left_target = if is_mouse_button_pressed(MouseButton::Left) && inside && !shift {
        cell_at(app, l, mx, my)
    } else {
        None
    };
    if let Some((x, y)) = left_target {
        let event = if app.game.cell(x, y).open {
            app.game.chord(x, y)
        } else {
            app.game.open(x, y)
        };
        event_sound(app, event);
    }

    let right_target = if is_mouse_button_pressed(MouseButton::Right) && inside {
        cell_at(app, l, mx, my)
    } else {
        None
    };
    if let Some((x, y)) = right_target {
        let event = app.game.toggle_flag(x, y);
        event_sound(app, event);
    }

    let (wx, wy) = mouse_wheel();
    if inside && (wx.abs() > 0.0 || wy.abs() > 0.0) {
        if ctrl {
            app.zoom = (app.zoom + wy.signum() * 0.1).clamp(0.5, 1.6);
        } else {
            app.cam.x += wx * l.cell * 2.0;
            app.cam.y += wy * l.cell * 2.0;
        }
    }
    if inside {
        let speed = l.cell;
        if is_key_down(KeyCode::Left) {
            app.cam.x -= speed;
        }
        if is_key_down(KeyCode::Right) {
            app.cam.x += speed;
        }
        if is_key_down(KeyCode::Up) {
            app.cam.y -= speed;
        }
        if is_key_down(KeyCode::Down) {
            app.cam.y += speed;
        }
    }

    let flag_key_target = if is_key_pressed(app.key_flag) {
        cell_at(app, l, mx, my)
    } else {
        None
    };
    if let Some((x, y)) = flag_key_target {
        let event = app.game.toggle_flag(x, y);
        event_sound(app, event);
    }

    let open_key_target = if is_key_pressed(app.key_open) {
        cell_at(app, l, mx, my)
    } else {
        None
    };
    if let Some((x, y)) = open_key_target {
        let event = if app.game.cell(x, y).open {
            app.game.chord(x, y)
        } else {
            app.game.open(x, y)
        };
        event_sound(app, event);
    }
}

fn key_label(key: KeyCode) -> String {
    match key {
        KeyCode::Space => "Space".to_owned(),
        KeyCode::Escape => "Esc".to_owned(),
        KeyCode::Enter => "Enter".to_owned(),
        KeyCode::LeftShift | KeyCode::RightShift => "Shift".to_owned(),
        KeyCode::LeftControl | KeyCode::RightControl => "Ctrl".to_owned(),
        KeyCode::LeftAlt | KeyCode::RightAlt => "Alt".to_owned(),
        _ => format!("{key:?}"),
    }
}

fn draw_settings(app: &mut App, sw: f32, sh: f32, mx: f32, my: f32) {
    let d = Dialog::new(sw, sh);
    draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.05, 0.05, 0.05, 0.46));
    bevel(d.x(0.0), d.y(0.0), d.w(Dialog::W), d.w(Dialog::H), false);
    let font = app.font.clone();
    let dark = Color::new(0.18, 0.18, 0.18, 1.0);
    centered(
        &font,
        "НАСТРОЙКИ ИГРЫ",
        d.x(Dialog::W / 2.0),
        d.y(38.0),
        d.fs(22.0),
        BLACK,
    );
    text(
        &font,
        "Классический вид, собственные ресурсы",
        d.x(28.0),
        d.y(62.0),
        d.fs(11.0),
        dark,
    );
    text(&font, "УРОВЕНЬ", d.x(28.0), d.y(96.0), d.fs(13.0), BLACK);

    let presets = [
        (Preset::Beginner, "Новичок · 10"),
        (Preset::Intermediate, "Любитель · 40"),
        (Preset::Expert, "Профи · 99"),
        (Preset::Custom, "Своя игра"),
    ];
    let gap = d.w(6.0);
    let pw = (d.w(Dialog::W) - d.w(56.0) - gap * 3.0) / 4.0;
    for (i, (preset, label)) in presets.iter().enumerate() {
        let xx = d.x(28.0) + i as f32 * (pw + gap);
        let yy = d.y(106.0);
        let hh = d.w(34.0);
        bevel(xx, yy, pw, hh, app.preset == *preset);
        centered(
            &font,
            label,
            xx + pw / 2.0,
            yy + d.w(22.0),
            d.fs(10.0),
            BLACK,
        );
        if is_mouse_button_pressed(MouseButton::Left) && hit(mx, my, xx, yy, pw, hh) {
            app.preset = *preset;
            app.close_edit();
            let (width, height) = preset_size(*preset);
            app.width = width;
            app.height = height;
            app.apply_board();
        }
    }

    text(
        &font,
        "РАЗМЕР ПОЛЯ",
        d.x(28.0),
        d.y(162.0),
        d.fs(13.0),
        BLACK,
    );
    field_row(app, &d, "Ширина (10–200)", 174.0, Edit::Width, mx, my);
    field_row(app, &d, "Высота (10–100)", 224.0, Edit::Height, mx, my);
    if app.preset == Preset::Custom {
        field_row(
            app,
            &d,
            "Мины (1 … площадь − 1)",
            274.0,
            Edit::Mines,
            mx,
            my,
        );
    } else {
        let mines = app.preset.fixed_mines().unwrap_or(app.custom_mines);
        text(
            &font,
            &format!("Мины: {mines} — фиксировано выбранным уровнем"),
            d.x(28.0),
            d.y(296.0),
            d.fs(12.0),
            dark,
        );
    }
    text(
        &font,
        "КЛАВИШИ УПРАВЛЕНИЯ",
        d.x(28.0),
        d.y(340.0),
        d.fs(13.0),
        BLACK,
    );
    let rows = [
        ("Флаг", Edit::KeyFlag, app.key_flag),
        ("Открыть клетку", Edit::KeyOpen, app.key_open),
    ];
    for (i, (name, edit, key)) in rows.iter().enumerate() {
        let yy = 352.0 + i as f32 * 38.0;
        text(&font, name, d.x(32.0), d.y(yy + 20.0), d.fs(12.0), BLACK);
        let xx = d.x(Dialog::W - 114.0);
        let bw = d.w(82.0);
        let bh = d.w(28.0);
        bevel(xx, d.y(yy), bw, bh, app.edit == Some(*edit));
        let label = if app.edit == Some(*edit) {
            "Нажмите…".to_owned()
        } else {
            key_label(*key)
        };
        centered(
            &font,
            &label,
            xx + bw / 2.0,
            d.y(yy) + d.w(19.0),
            d.fs(11.0),
            BLACK,
        );
        if is_mouse_button_pressed(MouseButton::Left) && hit(mx, my, xx, d.y(yy), bw, bh) {
            app.edit = Some(*edit);
            app.edit_buffer.clear();
        }
    }

    text(
        &font,
        "Тачпад: щелчок двумя пальцами = ПКМ. Shift+перетаскивание и колёсико — сдвиг.",
        d.x(28.0),
        d.y(Dialog::H - 68.0),
        d.fs(10.0),
        dark,
    );
    let bx = d.x(Dialog::W - 132.0);
    let by = d.y(Dialog::H - 46.0);
    let bw = d.w(104.0);
    let bh = d.w(29.0);
    bevel(bx, by, bw, bh, false);
    centered(
        &font,
        "Готово",
        bx + bw / 2.0,
        by + d.w(20.0),
        d.fs(13.0),
        BLACK,
    );
    if is_mouse_button_pressed(MouseButton::Left) && hit(mx, my, bx, by, bw, bh) {
        app.settings = false;
        app.close_edit();
    }

    if !matches!(app.edit, Some(Edit::KeyFlag) | Some(Edit::KeyOpen)) {
        return;
    }
    let Some(key) = get_last_key_pressed() else {
        return;
    };
    if key == KeyCode::Escape {
        app.close_edit();
        return;
    }
    let modifier = matches!(
        key,
        KeyCode::Unknown
            | KeyCode::Enter
            | KeyCode::LeftShift
            | KeyCode::RightShift
            | KeyCode::LeftControl
            | KeyCode::RightControl
    );
    if modifier {
        return;
    }
    match app.edit {
        Some(Edit::KeyFlag) => app.key_flag = key,
        Some(Edit::KeyOpen) => app.key_open = key,
        _ => {}
    }
    app.close_edit();
}

fn field_row(app: &mut App, d: &Dialog, label: &str, row_y: f32, edit: Edit, mx: f32, my: f32) {
    let font = app.font.clone();
    text(
        &font,
        label,
        d.x(28.0),
        d.y(row_y + 21.0),
        d.fs(12.0),
        BLACK,
    );
    let right = d.x(Dialog::W - 174.0);
    let field_w = d.w(72.0);
    let field_h = d.w(30.0);
    bevel(right, d.y(row_y), field_w, field_h, app.edit == Some(edit));
    let value = match edit {
        Edit::Width => app.width,
        Edit::Height => app.height,
        Edit::Mines => app.custom_mines,
        _ => 0,
    };
    let shown = if app.edit == Some(edit) && !app.edit_buffer.is_empty() {
        app.edit_buffer.clone()
    } else {
        value.to_string()
    };
    centered(
        &font,
        &shown,
        right + field_w / 2.0,
        d.y(row_y) + d.w(20.0),
        d.fs(14.0),
        BLACK,
    );
    for (i, (label, delta)) in [("−", -1isize), ("+", 1isize)].iter().enumerate() {
        let xx = right + field_w + d.w(7.0) + i as f32 * d.w(34.0);
        let bw = d.w(29.0);
        bevel(xx, d.y(row_y), bw, field_h, false);
        centered(
            &font,
            label,
            xx + bw / 2.0,
            d.y(row_y) + d.w(21.0),
            d.fs(17.0),
            BLACK,
        );
        if is_mouse_button_pressed(MouseButton::Left) && hit(mx, my, xx, d.y(row_y), bw, field_h) {
            app.close_edit();
            change_value(app, edit, *delta);
        }
    }
    if is_mouse_button_pressed(MouseButton::Left)
        && hit(mx, my, right, d.y(row_y), field_w, field_h)
    {
        app.edit = Some(edit);
        app.edit_buffer.clear();
    }
    if app.edit == Some(edit) {
        if is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::Escape) {
            app.close_edit();
        } else if is_key_pressed(KeyCode::Backspace) {
            app.edit_buffer.pop();
            apply_edit_buffer(app, edit);
        } else if let Some(c) = get_char_pressed() {
            if c.is_ascii_digit() {
                app.edit_buffer.push(c);
                apply_edit_buffer(app, edit);
            }
        }
    }
}

fn apply_edit_buffer(app: &mut App, edit: Edit) {
    let value = app.edit_buffer.parse::<usize>().unwrap_or(0).min(1_000_000);
    set_value(app, edit, value);
}

fn change_value(app: &mut App, edit: Edit, delta: isize) {
    let value = match edit {
        Edit::Width => app.width,
        Edit::Height => app.height,
        Edit::Mines => app.custom_mines,
        _ => 0,
    };
    let next = (value as isize + delta).max(1) as usize;
    set_value(app, edit, next);
}

fn set_value(app: &mut App, edit: Edit, value: usize) {
    match edit {
        Edit::Width => app.width = value.clamp(MIN_WIDTH, MAX_WIDTH),
        Edit::Height => app.height = value.clamp(MIN_HEIGHT, MAX_HEIGHT),
        Edit::Mines => app.custom_mines = value.max(1),
        _ => {}
    }
    app.width = app.width.clamp(MIN_WIDTH, MAX_WIDTH);
    app.height = app.height.clamp(MIN_HEIGHT, MAX_HEIGHT);
    app.custom_mines = app
        .custom_mines
        .clamp(1, Config::max_mines(app.width, app.height).min(MAX_MINES));
    app.apply_board();
}
