//! A headless harness for UI tests, built from `iced_test` parts (see `libraries.md`,
//! `iced_test`): the headless renderer, a `UserInterface` whose widget state persists across
//! rebuilds, the `iced_test::simulator` event builders, and real execution of the app's
//! `Task`s (outputs fed back into `update`, widget operations run against the live UI,
//! clipboard writes recorded).
#![allow(dead_code)]

pub mod fixtures;

use std::path::Path;

use archalign_desktop::app::{App, Boot, Message};
use archalign_desktop::view;
use iced::futures::StreamExt;
use iced::keyboard::{self, Key, Modifiers};
use iced::{Event, Point, Size, Task, mouse};
use iced_test::core::renderer::Headless;
use iced_test::core::widget::Operation as _;
use iced_test::core::widget::operation::{self, Outcome};
use iced_test::core::{clipboard, event, widget};
use iced_test::runtime::{Action, UserInterface, task, user_interface};
use iced_test::selector::Selector;

type Renderer = iced_test::renderer::Renderer;

/// The test window size: the spec's default 1440×900.
pub const WINDOW: Size = Size::new(1440.0, 900.0);

pub struct Harness {
    pub app: App,
    renderer: Renderer,
    cache: Option<user_interface::Cache>,
    pub size: Size,
    cursor: mouse::Cursor,
    /// Every clipboard write, in order.
    pub clipboard: Vec<String>,
    /// Every message dispatched, in order (for ordering assertions).
    pub log: Vec<String>,
    rt: tokio::runtime::Runtime,
}

impl Harness {
    /// Boots the app (running the boot task to completion).
    pub fn new(boot: Boot) -> Self {
        archalign_desktop::fonts::ensure_loaded();
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("tokio runtime");
        let renderer = rt
            .block_on(Renderer::new(
                archalign_desktop::fonts::MONO,
                iced::Pixels(16.0),
                None,
            ))
            .expect("headless renderer");
        let (mut app, boot_task) = App::new(boot);
        app.window = WINDOW;
        let mut h = Self {
            app,
            renderer,
            cache: Some(user_interface::Cache::default()),
            size: WINDOW,
            cursor: mouse::Cursor::Unavailable,
            clipboard: Vec::new(),
            log: Vec::new(),
            rt,
        };
        h.run(boot_task);
        h
    }

    /// Dispatches a message and runs the resulting task to completion.
    pub fn send(&mut self, message: Message) {
        self.log.push(format!("{message:?}"));
        let task = self.app.update(message);
        self.run(task);
    }

    /// Runs a task to completion, dispatching its outputs.
    pub fn run_task(&mut self, task: Task<Message>) {
        self.run(task);
    }

    fn run(&mut self, task: Task<Message>) {
        let Some(mut stream) = task::into_stream(task) else {
            return;
        };
        while let Some(action) = self.rt.block_on(stream.next()) {
            match action {
                Action::Output(message) => self.send(message),
                Action::Widget(op) => self.operate(op),
                Action::Clipboard(iced_test::runtime::clipboard::Action::Write {
                    contents,
                    ..
                }) => self.clipboard.push(contents),
                _ => {}
            }
        }
    }

    fn with_ui<T>(
        &mut self,
        f: impl FnOnce(&mut UserInterface<'_, Message, iced::Theme, Renderer>, &mut Renderer) -> T,
    ) -> T {
        let cache = self.cache.take().unwrap_or_default();
        let element = view::view(&self.app);
        let mut ui = UserInterface::build(element, self.size, cache, &mut self.renderer);
        let out = f(&mut ui, &mut self.renderer);
        self.cache = Some(ui.into_cache());
        out
    }

    fn operate(&mut self, op: Box<dyn widget::Operation>) {
        // Like the runtime, rebuild the interface for the latest state before operating.
        self.with_ui(|_, _| ());
        self.with_ui(|ui, renderer| {
            let mut current = Some(op);
            while let Some(mut op) = current.take() {
                ui.operate(renderer, &mut op);
                if let Outcome::Chain(next) = op.finish() {
                    current = Some(next);
                }
            }
        });
    }

    /// Feeds raw events through the UI, then dispatches the produced messages. Key presses
    /// reach the app with their capture status, like the `event::listen_with` subscription.
    pub fn simulate(&mut self, events: Vec<Event>) -> Vec<event::Status> {
        let cursor = self.cursor;
        let mut messages = Vec::new();
        let statuses = self.with_ui(|ui, renderer| {
            let (_, statuses) = ui.update(
                &events,
                cursor,
                renderer,
                &mut clipboard::Null,
                &mut messages,
            );
            statuses
        });
        for m in messages {
            self.send(m);
        }
        for (e, s) in events.iter().zip(&statuses) {
            if let Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) = e {
                self.send(Message::KeyPressed(
                    key.clone(),
                    *modifiers,
                    *s == event::Status::Captured,
                ));
            }
        }
        statuses
    }

    /// Finds a widget.
    pub fn find<S>(&mut self, selector: S) -> Option<S::Output>
    where
        S: Selector + Send,
        S::Output: Clone + Send,
    {
        self.with_ui(|ui, renderer| {
            let mut op = selector.find();
            ui.operate(renderer, &mut operation::black_box(&mut op));
            match op.finish() {
                Outcome::Some(found) => found,
                _ => None,
            }
        })
    }

    /// Every widget whose ID starts with `prefix`, with its visible bounds.
    pub fn ids_with_prefix(&mut self, prefix: &str) -> Vec<(String, Option<iced::Rectangle>)> {
        let prefix = prefix.to_string();
        self.with_ui(|ui, renderer| {
            let selector = move |c: iced_test::selector::Candidate<'_>| {
                let id = format!("{:?}", c.id()?);
                // `Id(Custom("tree-row:src"))`
                let inner = id.split('"').nth(1)?.to_string();
                inner
                    .starts_with(&prefix)
                    .then(|| (inner, c.visible_bounds()))
            };
            let mut op = selector.find_all();
            ui.operate(renderer, &mut operation::black_box(&mut op));
            match op.finish() {
                Outcome::Some(all) => all,
                _ => Vec::new(),
            }
        })
    }

    /// Whether a widget with this ID is in the view.
    pub fn has(&mut self, id: impl Into<widget::Id>) -> bool {
        self.find(id.into()).is_some()
    }

    /// Bounds of a widget.
    pub fn bounds(&mut self, id: impl Into<widget::Id>) -> Option<iced::Rectangle> {
        self.find(id.into()).and_then(|t| t.visible_bounds())
    }

    /// Moves the cursor (with a `CursorMoved` event).
    pub fn move_to(&mut self, p: Point) {
        self.cursor = mouse::Cursor::Available(p);
        self.simulate(vec![Event::Mouse(mouse::Event::CursorMoved {
            position: p,
        })]);
    }

    /// Clicks at a point.
    pub fn click_at(&mut self, p: Point) {
        self.move_to(p);
        self.simulate(iced_test::simulator::click().collect());
    }

    /// Clicks the center of a widget; panics if it isn't there.
    pub fn click(&mut self, id: impl Into<widget::Id>) {
        let id = id.into();
        let b = self
            .bounds(id.clone())
            .unwrap_or_else(|| panic!("widget {id:?} not found or not visible"));
        self.click_at(b.center());
    }

    /// Clicks the first widget containing this text.
    pub fn click_text(&mut self, text: &str) {
        let t = self
            .find(text)
            .unwrap_or_else(|| panic!("text {text:?} not found"));
        let b = t.visible_bounds().expect("text visible");
        self.click_at(b.center());
    }

    /// Whether some widget shows this exact text.
    pub fn has_text(&mut self, text: &str) -> bool {
        self.find(text).is_some()
    }

    /// Presses and releases a key with modifiers.
    pub fn key(&mut self, key: Key, modifiers: Modifiers) {
        let text = match &key {
            Key::Character(c) if !modifiers.command() => Some(c.clone()),
            _ => None,
        };
        let press = Event::Keyboard(keyboard::Event::KeyPressed {
            key: key.clone(),
            modified_key: key.clone(),
            physical_key: keyboard::key::Physical::Unidentified(
                keyboard::key::NativeCode::Unidentified,
            ),
            location: keyboard::Location::Standard,
            modifiers,
            text,
            repeat: false,
        });
        let release = Event::Keyboard(keyboard::Event::KeyReleased {
            key: key.clone(),
            modified_key: key,
            physical_key: keyboard::key::Physical::Unidentified(
                keyboard::key::NativeCode::Unidentified,
            ),
            location: keyboard::Location::Standard,
            modifiers,
        });
        self.simulate(vec![
            Event::Keyboard(keyboard::Event::ModifiersChanged(modifiers)),
            press,
            release,
            Event::Keyboard(keyboard::Event::ModifiersChanged(Modifiers::default())),
        ]);
    }

    /// ⌘ + character.
    pub fn cmd(&mut self, c: &str) {
        self.key(Key::Character(c.into()), Modifiers::COMMAND);
    }

    /// ⌘⇧ + character.
    pub fn cmd_shift(&mut self, c: &str) {
        self.key(
            Key::Character(c.into()),
            Modifiers::COMMAND | Modifiers::SHIFT,
        );
    }

    /// Types text into the focused widget.
    pub fn type_text(&mut self, text: &str) {
        self.simulate(iced_test::simulator::typewrite(text).collect());
    }

    /// Scrolls with the wheel at the current cursor position.
    pub fn wheel(&mut self, dy: f32) {
        self.simulate(vec![Event::Mouse(mouse::Event::WheelScrolled {
            delta: mouse::ScrollDelta::Pixels { x: 0.0, y: dy },
        })]);
    }

    /// Renders the current UI and writes it as a PNG.
    pub fn screenshot(&mut self, path: &Path) {
        let theme = self.app.iced_theme();
        let cursor = self.cursor;
        let size = self.size;
        let (rgba, w, h) = self.with_ui(|ui, renderer| {
            let mut messages = Vec::new();
            let _ = ui.update(
                &[Event::Window(iced::window::Event::RedrawRequested(
                    iced::time::Instant::now(),
                ))],
                cursor,
                renderer,
                &mut clipboard::Null,
                &mut messages,
            );
            let base = iced::theme::Base::base(&theme);
            ui.draw(
                renderer,
                &theme,
                &iced_test::core::renderer::Style {
                    text_color: base.text_color,
                },
                cursor,
            );
            let scale = 2.0;
            let (w, h) = ((size.width * scale) as u32, (size.height * scale) as u32);
            let rgba = renderer.screenshot(iced::Size::new(w, h), scale, base.background_color);
            (rgba, w, h)
        });
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).expect("screenshot dir");
        }
        let file = std::fs::File::create(path).expect("screenshot file");
        let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w, h);
        enc.set_color(png::ColorType::Rgba);
        let mut writer = enc.write_header().expect("png header");
        writer.write_image_data(&rgba).expect("png data");
    }
}

/// `target/smoke/<name>` inside the workspace.
pub fn smoke_dir(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/smoke")
        .join(name)
}
