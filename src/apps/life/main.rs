#![feature(thread_sleep_until)]

use std::borrow::Cow;
use std::collections::VecDeque;
use std::fmt::Debug;
use std::sync::mpsc::{self, TryRecvError};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use grid::Grid;
use iced_runtime::Program;
use iced_widget::core::Length;
use iced_widget::core::alignment::Vertical;
use iced_widget::runtime::Task;
use iced_widget::{button, checkbox, column, container, pick_list, row, slider, text};

mod preset;
use preset::Preset;

use helenos_iced::{Element, ProgramExt, SendMsgFn, WindowOptions};

impl Program for GameOfLife {
    type Message = Message;
    type Renderer = iced_widget::Renderer;
    type Theme = iced_widget::Theme;

    fn update(&mut self, message: Self::Message) -> Task<Self::Message> {
        self.update(message)
    }

    fn view(&self) -> Element<Self::Message> {
        self.view()
    }
}

impl ProgramExt for GameOfLife {
    fn stop(&self) {
        self.worker_sender.send(WorkerMessage::Stop).unwrap();
        *self.looper_state.0.lock().unwrap() = LooperState::Stop;
        self.looper_state.1.notify_all();

        if let Some(handles) = self.join_handles.lock().unwrap().take() {
            for handle in handles {
                handle.join().unwrap();
            }
        }
    }
}

enum WorkerMessage {
    Stop,
    Work(Box<dyn FnOnce() -> Option<Message> + Send>),
}

fn worker(worker_receiver: mpsc::Receiver<WorkerMessage>, send_msg: SendMsgFn<Message>) {
    let mut pending_msgs = VecDeque::new();
    loop {
        // iterate all pending messages to find if there is any stop
        loop {
            match worker_receiver.try_recv() {
                Ok(WorkerMessage::Stop) => return,
                Ok(WorkerMessage::Work(task)) => {
                    pending_msgs.push_back(task);
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    panic!("Worker channel disconnected");
                }
            }
        }
        if pending_msgs.is_empty() {
            // blocking recv
            match worker_receiver.recv().unwrap() {
                WorkerMessage::Stop => return,
                WorkerMessage::Work(task) => {
                    pending_msgs.push_back(task);
                }
            }
        }
        let task = pending_msgs.pop_front().unwrap();
        if let Some(msg) = task() {
            send_msg(msg);
        }
    }
}

#[derive(Debug)]
enum LooperState {
    Paused,
    Running { speed: usize },
    Stop,
}

fn looper(looper_state: Arc<(Mutex<LooperState>, Condvar)>, send_msg: SendMsgFn<Message>) {
    let mut last_tick = None;
    let (looper_state, condvar) = &*looper_state;
    let mut state = looper_state.lock().unwrap();
    loop {
        let speed = match *state {
            LooperState::Paused => {
                state = condvar.wait(state).unwrap();
                continue;
            }
            LooperState::Stop => return,
            LooperState::Running { speed } => speed,
        };
        drop(state);
        if let Some(last_tick) = last_tick {
            std::thread::sleep_until(last_tick + Duration::from_millis(1000 / speed as u64));
        }
        last_tick = Some(Instant::now());
        send_msg(Message::Tick);
        state = looper_state.lock().unwrap();
    }
}

struct GameOfLife {
    grid: Grid,
    is_playing: bool,
    queued_ticks: usize,
    speed: usize,
    version: usize,

    worker_sender: mpsc::Sender<WorkerMessage>,
    looper_state: Arc<(Mutex<LooperState>, Condvar)>,

    join_handles: Mutex<Option<[JoinHandle<()>; 2]>>,
}

impl Debug for GameOfLife {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GameOfLife")
            .field("is_playing", &self.is_playing)
            .field("queued_ticks", &self.queued_ticks)
            .field("speed", &self.speed)
            .field("version", &self.version)
            .field("looper_state", &self.looper_state)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone)]
enum Message {
    Grid(grid::Message, usize),
    Tick,
    TogglePlayback,
    ToggleGrid(bool),
    Next,
    Clear,
    SpeedChanged(f32),
    PresetPicked(Preset),
}

impl GameOfLife {
    fn new(create_send_msg: impl Fn() -> SendMsgFn<Message>) -> Self {
        let (worker_sender, worker_receiver) = mpsc::channel();
        let worker_handle = std::thread::spawn({
            let send_msg = create_send_msg();
            move || worker(worker_receiver, send_msg)
        });
        let looper_state = Arc::new((Mutex::new(LooperState::Paused), Condvar::new()));
        let looper_handle = std::thread::spawn({
            let looper_state = looper_state.clone();
            let send_msg = create_send_msg();
            move || looper(looper_state, send_msg)
        });
        Self {
            grid: Grid::default(),
            is_playing: false,
            queued_ticks: 0,
            speed: 5,
            version: 0,
            worker_sender,
            looper_state,
            join_handles: Mutex::new(Some([worker_handle, looper_handle])),
        }
    }

    fn update_looper_state(&mut self) {
        let mut guard = self.looper_state.0.lock().unwrap();
        if !matches!(*guard, LooperState::Stop) {
            if self.is_playing {
                *guard = LooperState::Running { speed: self.speed };
            } else {
                *guard = LooperState::Paused;
            }
        }
        self.looper_state.1.notify_all();
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Grid(message, version) => {
                if version == self.version {
                    self.grid.update(message);
                }
            }
            Message::Tick | Message::Next => {
                self.queued_ticks = (self.queued_ticks + 1).min(self.speed);

                if let Some(task) = self.grid.tick(self.queued_ticks) {
                    self.queued_ticks = 0;

                    let version = self.version;
                    self.worker_sender
                        .send(WorkerMessage::Work(Box::new(move || {
                            let message = task();
                            Some(Message::Grid(message, version))
                        })))
                        .unwrap();
                }
            }
            Message::TogglePlayback => {
                self.is_playing = !self.is_playing;
                self.update_looper_state();
            }
            Message::ToggleGrid(show_grid_lines) => {
                self.grid.toggle_lines(show_grid_lines);
            }
            Message::Clear => {
                self.grid.clear();
                self.version += 1;
            }
            Message::SpeedChanged(speed) => {
                self.speed = speed.round() as usize;
                if self.is_playing {
                    self.update_looper_state();
                }
            }
            Message::PresetPicked(new_preset) => {
                self.grid = Grid::from_preset(new_preset, self.grid.scaling());
                self.version += 1;
            }
        }

        if self.is_playing {}

        Task::none()
    }

    fn view(&self) -> Element<Message> {
        let version = self.version;
        let controls = self.view_controls();

        let content = column![
            self.grid
                .view()
                .map(move |message| Message::Grid(message, version)),
            controls,
        ]
        .height(Length::Fill);

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn view_controls(&self) -> Element<Message> {
        let playback_controls = row![
            button(if self.is_playing { "Pause" } else { "Play" })
                .on_press(Message::TogglePlayback),
            button("Next")
                .on_press(Message::Next)
                .style(button::secondary),
        ]
        .spacing(10);

        let speed_controls = row![
            slider(1.0..=1000.0, self.speed as f32, Message::SpeedChanged),
            text!("x{}", self.speed).size(16),
        ]
        .align_y(Vertical::Center)
        .spacing(10);

        let version = self.version;
        let scaling = self.grid.scaling();
        let scaling = row![
            text!("Zoom: ").size(16),
            slider(
                grid::Grid::MIN_SCALING..=grid::Grid::MAX_SCALING,
                scaling,
                move |s| Message::Grid(grid::Message::Scaled(s, None), version),
            )
            .step(0.1),
            text!("x{:.1}", scaling).size(16)
        ]
        .align_y(Vertical::Center)
        .spacing(10);

        column![
            row![
                playback_controls,
                speed_controls,
                checkbox("Grid", self.grid.are_lines_visible()).on_toggle(Message::ToggleGrid),
                row![
                    pick_list(preset::ALL, Some(self.grid.preset()), Message::PresetPicked),
                    button("Clear")
                        .on_press(Message::Clear)
                        .style(button::danger)
                ]
                .spacing(10)
            ]
            .spacing(20)
            .align_y(Vertical::Center),
            scaling,
        ]
        .padding(10)
        .spacing(20)
        .into()
    }
}

mod grid {
    use iced_widget::canvas::event::{self, Event};
    use iced_widget::canvas::{self, Cache, Frame, Geometry, Path, Text};
    use iced_widget::core::{
        Color, Length, Point, Rectangle, Size, Vector, alignment, mouse, touch,
    };
    use iced_widget::{Canvas, Renderer, Theme};

    use crate::Element;

    use super::Preset;
    use std::collections::{HashMap, HashSet};
    use std::ops::RangeInclusive;
    use std::time::{Duration, Instant};

    pub struct Grid {
        state: State,
        preset: Preset,
        life_cache: Cache,
        grid_cache: Cache,
        translation: Vector,
        scaling: f32,
        show_lines: bool,
        last_tick_duration: Duration,
        last_queued_ticks: usize,
    }

    #[derive(Debug, Clone)]
    pub enum Message {
        Populate(Cell),
        Unpopulate(Cell),
        Translated(Vector),
        Scaled(f32, Option<Vector>),
        Ticked { life: Life, tick_duration: Duration },
    }

    impl Default for Grid {
        fn default() -> Self {
            Self::from_preset(Preset::default(), 0.5)
        }
    }

    impl Grid {
        pub const MIN_SCALING: f32 = 0.1;
        pub const MAX_SCALING: f32 = 2.0;

        pub fn from_preset(preset: Preset, scaling: f32) -> Self {
            Self {
                state: State::with_life(
                    preset
                        .life()
                        .into_iter()
                        .map(|(i, j)| Cell { i, j })
                        .collect(),
                ),
                preset,
                life_cache: Cache::default(),
                grid_cache: Cache::default(),
                translation: Vector::default(),
                scaling,
                show_lines: true,
                last_tick_duration: Duration::default(),
                last_queued_ticks: 0,
            }
        }

        pub fn tick(&mut self, amount: usize) -> Option<impl FnOnce() -> Message + 'static> {
            let tick_fn = self.state.create_tick_fn(amount)?;

            self.last_queued_ticks = amount;

            Some(move || {
                let start = Instant::now();
                let result = tick_fn();
                let tick_duration = start.elapsed() / amount as u32;

                Message::Ticked {
                    life: result,
                    tick_duration,
                }
            })
        }

        pub fn update(&mut self, message: Message) {
            match message {
                Message::Populate(cell) => {
                    self.state.populate(cell);
                    self.life_cache.clear();

                    self.preset = Preset::Custom;
                }
                Message::Unpopulate(cell) => {
                    self.state.unpopulate(&cell);
                    self.life_cache.clear();

                    self.preset = Preset::Custom;
                }
                Message::Translated(translation) => {
                    self.translation = translation;

                    self.life_cache.clear();
                    self.grid_cache.clear();
                }
                Message::Scaled(scaling, translation) => {
                    self.scaling = scaling;

                    if let Some(translation) = translation {
                        self.translation = translation;
                    }

                    self.life_cache.clear();
                    self.grid_cache.clear();
                }
                Message::Ticked {
                    life,
                    tick_duration,
                } => {
                    self.state.update(life);
                    self.life_cache.clear();

                    self.last_tick_duration = tick_duration;
                }
            }
        }

        pub fn view(&self) -> Element<Message> {
            Canvas::new(self)
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
        }

        pub fn clear(&mut self) {
            self.state = State::default();
            self.preset = Preset::Custom;

            self.life_cache.clear();
        }

        pub fn preset(&self) -> Preset {
            self.preset
        }

        pub fn toggle_lines(&mut self, enabled: bool) {
            self.show_lines = enabled;
        }

        pub fn are_lines_visible(&self) -> bool {
            self.show_lines
        }

        pub fn scaling(&self) -> f32 {
            self.scaling
        }

        fn visible_region(&self, size: Size) -> Region {
            let width = size.width / self.scaling;
            let height = size.height / self.scaling;

            Region {
                x: -self.translation.x - width / 2.0,
                y: -self.translation.y - height / 2.0,
                width,
                height,
            }
        }

        fn project(&self, position: Point, size: Size) -> Point {
            let region = self.visible_region(size);

            Point::new(
                position.x / self.scaling + region.x,
                position.y / self.scaling + region.y,
            )
        }
    }

    impl canvas::Program<Message> for Grid {
        type State = Interaction;

        fn update(
            &self,
            interaction: &mut Interaction,
            event: Event,
            bounds: Rectangle,
            cursor: mouse::Cursor,
        ) -> (event::Status, Option<Message>) {
            if let Event::Mouse(mouse::Event::ButtonReleased(_)) = event {
                *interaction = Interaction::None;
            }

            let Some(cursor_position) = cursor.position_in(bounds) else {
                return (event::Status::Ignored, None);
            };

            let cell = Cell::at(self.project(cursor_position, bounds.size()));
            let is_populated = self.state.contains(&cell);

            let (populate, unpopulate) = if is_populated {
                (None, Some(Message::Unpopulate(cell)))
            } else {
                (Some(Message::Populate(cell)), None)
            };

            match event {
                Event::Touch(touch::Event::FingerMoved { .. }) => {
                    let message = {
                        *interaction = if is_populated {
                            Interaction::Erasing
                        } else {
                            Interaction::Drawing
                        };

                        populate.or(unpopulate)
                    };

                    (event::Status::Captured, message)
                }
                Event::Mouse(mouse_event) => match mouse_event {
                    mouse::Event::ButtonPressed(button) => {
                        let message = match button {
                            mouse::Button::Left => {
                                *interaction = if is_populated {
                                    Interaction::Erasing
                                } else {
                                    Interaction::Drawing
                                };

                                populate.or(unpopulate)
                            }
                            mouse::Button::Right => {
                                *interaction = Interaction::Panning {
                                    translation: self.translation,
                                    start: cursor_position,
                                };

                                None
                            }
                            _ => None,
                        };

                        (event::Status::Captured, message)
                    }
                    mouse::Event::CursorMoved { .. } => {
                        let message = match *interaction {
                            Interaction::Drawing => populate,
                            Interaction::Erasing => unpopulate,
                            Interaction::Panning { translation, start } => {
                                Some(Message::Translated(
                                    translation + (cursor_position - start) * (1.0 / self.scaling),
                                ))
                            }
                            Interaction::None => None,
                        };

                        let event_status = match interaction {
                            Interaction::None => event::Status::Ignored,
                            _ => event::Status::Captured,
                        };

                        (event_status, message)
                    }
                    mouse::Event::WheelScrolled { delta } => match delta {
                        mouse::ScrollDelta::Lines { y, .. }
                        | mouse::ScrollDelta::Pixels { y, .. } => {
                            if y < 0.0 && self.scaling > Self::MIN_SCALING
                                || y > 0.0 && self.scaling < Self::MAX_SCALING
                            {
                                let old_scaling = self.scaling;

                                let scaling = (self.scaling * (1.0 + y / 30.0))
                                    .clamp(Self::MIN_SCALING, Self::MAX_SCALING);

                                let translation = if let Some(cursor_to_center) =
                                    cursor.position_from(bounds.center())
                                {
                                    let factor = scaling - old_scaling;

                                    Some(
                                        self.translation
                                            - Vector::new(
                                                cursor_to_center.x * factor
                                                    / (old_scaling * old_scaling),
                                                cursor_to_center.y * factor
                                                    / (old_scaling * old_scaling),
                                            ),
                                    )
                                } else {
                                    None
                                };

                                (
                                    event::Status::Captured,
                                    Some(Message::Scaled(scaling, translation)),
                                )
                            } else {
                                (event::Status::Captured, None)
                            }
                        }
                    },
                    _ => (event::Status::Ignored, None),
                },
                _ => (event::Status::Ignored, None),
            }
        }

        fn draw(
            &self,
            _interaction: &Interaction,
            renderer: &Renderer,
            _theme: &Theme,
            bounds: Rectangle,
            cursor: mouse::Cursor,
        ) -> Vec<Geometry> {
            let center = Vector::new(bounds.width / 2.0, bounds.height / 2.0);

            let life = self.life_cache.draw(renderer, bounds.size(), |frame| {
                let background = Path::rectangle(Point::ORIGIN, frame.size());
                frame.fill(&background, Color::from_rgb8(0x40, 0x44, 0x4B));

                frame.with_save(|frame| {
                    frame.translate(center);
                    frame.scale(self.scaling);
                    frame.translate(self.translation);
                    frame.scale(Cell::SIZE);

                    let region = self.visible_region(frame.size());

                    for cell in region.cull(self.state.cells()) {
                        frame.fill_rectangle(
                            Point::new(cell.j as f32, cell.i as f32),
                            Size::UNIT,
                            Color::WHITE,
                        );
                    }
                });
            });

            let overlay = {
                let mut frame = Frame::new(renderer, bounds.size());

                let hovered_cell = cursor
                    .position_in(bounds)
                    .map(|position| Cell::at(self.project(position, frame.size())));

                if let Some(cell) = hovered_cell {
                    frame.with_save(|frame| {
                        frame.translate(center);
                        frame.scale(self.scaling);
                        frame.translate(self.translation);
                        frame.scale(Cell::SIZE);

                        frame.fill_rectangle(
                            Point::new(cell.j as f32, cell.i as f32),
                            Size::UNIT,
                            Color {
                                a: 0.5,
                                ..Color::BLACK
                            },
                        );
                    });
                }

                let text = Text {
                    color: Color::WHITE,
                    size: 14.0.into(),
                    position: Point::new(frame.width(), frame.height()),
                    horizontal_alignment: alignment::Horizontal::Right,
                    vertical_alignment: alignment::Vertical::Bottom,
                    ..Text::default()
                };

                if let Some(cell) = hovered_cell {
                    frame.fill_text(Text {
                        content: format!("({}, {})", cell.j, cell.i),
                        position: text.position - Vector::new(0.0, 16.0),
                        ..text
                    });
                }

                let cell_count = self.state.cell_count();

                frame.fill_text(Text {
                    content: format!(
                        "{cell_count} cell{} @ {:?} ({})",
                        if cell_count == 1 { "" } else { "s" },
                        self.last_tick_duration,
                        self.last_queued_ticks
                    ),
                    ..text
                });

                frame.into_geometry()
            };

            if self.scaling >= 0.2 && self.show_lines {
                let grid = self.grid_cache.draw(renderer, bounds.size(), |frame| {
                    frame.translate(center);
                    frame.scale(self.scaling);
                    frame.translate(self.translation);
                    frame.scale(Cell::SIZE);

                    let region = self.visible_region(frame.size());
                    let rows = region.rows();
                    let columns = region.columns();
                    let (total_rows, total_columns) =
                        (rows.clone().count(), columns.clone().count());
                    let width = 2.0 / Cell::SIZE as f32;
                    let color = Color::from_rgb8(70, 74, 83);

                    frame.translate(Vector::new(-width / 2.0, -width / 2.0));

                    for row in region.rows() {
                        frame.fill_rectangle(
                            Point::new(*columns.start() as f32, row as f32),
                            Size::new(total_columns as f32, width),
                            color,
                        );
                    }

                    for column in region.columns() {
                        frame.fill_rectangle(
                            Point::new(column as f32, *rows.start() as f32),
                            Size::new(width, total_rows as f32),
                            color,
                        );
                    }
                });

                vec![life, grid, overlay]
            } else {
                vec![life, overlay]
            }
        }

        fn mouse_interaction(
            &self,
            interaction: &Interaction,
            bounds: Rectangle,
            cursor: mouse::Cursor,
        ) -> mouse::Interaction {
            match interaction {
                Interaction::Drawing => mouse::Interaction::Crosshair,
                Interaction::Erasing => mouse::Interaction::Crosshair,
                Interaction::Panning { .. } => mouse::Interaction::Grabbing,
                Interaction::None if cursor.is_over(bounds) => mouse::Interaction::Crosshair,
                Interaction::None => mouse::Interaction::default(),
            }
        }
    }

    #[derive(Default)]
    struct State {
        life: Life,
        births: HashSet<Cell>,
        is_ticking: bool,
    }

    impl State {
        pub fn with_life(life: Life) -> Self {
            Self {
                life,
                ..Self::default()
            }
        }

        fn cell_count(&self) -> usize {
            self.life.len() + self.births.len()
        }

        fn contains(&self, cell: &Cell) -> bool {
            self.life.contains(cell) || self.births.contains(cell)
        }

        fn cells(&self) -> impl Iterator<Item = &Cell> {
            self.life.iter().chain(self.births.iter())
        }

        fn populate(&mut self, cell: Cell) {
            if self.is_ticking {
                self.births.insert(cell);
            } else {
                self.life.populate(cell);
            }
        }

        fn unpopulate(&mut self, cell: &Cell) {
            if self.is_ticking {
                let _ = self.births.remove(cell);
            } else {
                self.life.unpopulate(cell);
            }
        }

        fn update(&mut self, mut life: Life) {
            self.births.drain().for_each(|cell| life.populate(cell));

            self.life = life;
            self.is_ticking = false;
        }

        fn create_tick_fn(&mut self, amount: usize) -> Option<impl FnOnce() -> Life + 'static> {
            if self.is_ticking {
                return None;
            }

            self.is_ticking = true;

            let mut life = self.life.clone();

            Some(move || {
                for _ in 0..amount {
                    life.tick();
                }

                life
            })
        }
    }

    #[derive(Clone, Default)]
    pub struct Life {
        cells: HashSet<Cell>,
    }

    impl Life {
        fn len(&self) -> usize {
            self.cells.len()
        }

        fn contains(&self, cell: &Cell) -> bool {
            self.cells.contains(cell)
        }

        fn populate(&mut self, cell: Cell) {
            self.cells.insert(cell);
        }

        fn unpopulate(&mut self, cell: &Cell) {
            let _ = self.cells.remove(cell);
        }

        fn tick(&mut self) {
            let mut adjacent_life: HashMap<Cell, i32> = HashMap::default();

            for cell in &self.cells {
                let _ = adjacent_life.entry(*cell).or_insert(0);

                for neighbor in Cell::neighbors(*cell) {
                    let amount = adjacent_life.entry(neighbor).or_insert(0);

                    *amount += 1;
                }
            }

            for (cell, amount) in &adjacent_life {
                match amount {
                    2 => {}
                    3 => {
                        let _ = self.cells.insert(*cell);
                    }
                    _ => {
                        let _ = self.cells.remove(cell);
                    }
                }
            }
        }

        pub fn iter(&self) -> impl Iterator<Item = &Cell> {
            self.cells.iter()
        }
    }

    impl std::iter::FromIterator<Cell> for Life {
        fn from_iter<I: IntoIterator<Item = Cell>>(iter: I) -> Self {
            Life {
                cells: iter.into_iter().collect(),
            }
        }
    }

    impl std::fmt::Debug for Life {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("Life")
                .field("cells", &self.cells.len())
                .finish()
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct Cell {
        i: isize,
        j: isize,
    }

    impl Cell {
        const SIZE: u16 = 20;

        fn at(position: Point) -> Cell {
            let i = (position.y / Cell::SIZE as f32).ceil() as isize;
            let j = (position.x / Cell::SIZE as f32).ceil() as isize;

            Cell {
                i: i.saturating_sub(1),
                j: j.saturating_sub(1),
            }
        }

        fn cluster(cell: Cell) -> impl Iterator<Item = Cell> {
            use itertools::Itertools;

            let rows = cell.i.saturating_sub(1)..=cell.i.saturating_add(1);
            let columns = cell.j.saturating_sub(1)..=cell.j.saturating_add(1);

            rows.cartesian_product(columns).map(|(i, j)| Cell { i, j })
        }

        fn neighbors(cell: Cell) -> impl Iterator<Item = Cell> {
            Cell::cluster(cell).filter(move |candidate| *candidate != cell)
        }
    }

    pub struct Region {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    }

    impl Region {
        fn rows(&self) -> RangeInclusive<isize> {
            let first_row = (self.y / Cell::SIZE as f32).floor() as isize;

            let visible_rows = (self.height / Cell::SIZE as f32).ceil() as isize;

            first_row..=first_row + visible_rows
        }

        fn columns(&self) -> RangeInclusive<isize> {
            let first_column = (self.x / Cell::SIZE as f32).floor() as isize;

            let visible_columns = (self.width / Cell::SIZE as f32).ceil() as isize;

            first_column..=first_column + visible_columns
        }

        fn cull<'a>(
            &self,
            cells: impl Iterator<Item = &'a Cell>,
        ) -> impl Iterator<Item = &'a Cell> {
            let rows = self.rows();
            let columns = self.columns();

            cells.filter(move |cell| rows.contains(&cell.i) && columns.contains(&cell.j))
        }
    }

    pub enum Interaction {
        None,
        Drawing,
        Erasing,
        Panning { translation: Vector, start: Point },
    }

    impl Default for Interaction {
        fn default() -> Self {
            Self::None
        }
    }
}

fn main() {
    helenos_iced::run(
        |create_send_msg| GameOfLife::new(create_send_msg),
        WindowOptions {
            caption: Cow::Borrowed("Game of Life"),
            maximized: true,
        },
    );
}
