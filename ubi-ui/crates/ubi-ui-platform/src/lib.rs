//! winit desktop host. Component, layout and rendering logic stay platform-neutral.
pub mod motion;
mod timing;
pub use motion::MotionPreference;
use std::{
    path::Path,
    sync::Arc,
    time::{Duration, Instant},
};
use ubi_ui_core::motion::{MotionFrame, MotionState};
pub use ubi_ui_core::{Changes, Error, Tree, View};
use ubi_ui_core::{Effect, Kind, OverlayKind};
pub use ubi_ui_input::Event;
pub use ubi_ui_input::{Input, Key as UiKey};
use ubi_ui_layout::present;
pub use ubi_ui_layout::{Layout, PresentedScene, Scene, TextMeasure};
use ubi_ui_render::{Frame, Renderer};
use ubi_ui_theme::Theme;
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::{ElementState, Ime, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key, ModifiersState, NamedKey},
    window::{Window, WindowId},
};

/// Application-owned state. Dispatch validates before committing; returned
/// unmounts let the host release keyed focus/capture resources immediately.
pub trait Application {
    fn view(&self) -> &View;
    fn dispatch(&mut self, event: &Event) -> Result<Changes, Error>;
}

#[derive(Clone, Debug)]
pub struct Options {
    pub title: String,
    pub width: u32,
    pub height: u32,
    pub smoke: bool,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            title: "Ubi UI".into(),
            width: 1100,
            height: 820,
            smoke: false,
        }
    }
}

pub fn run(app: impl Application + 'static, options: Options) -> Result<(), Error> {
    run_with_motion(app, options, MotionPreference::System)
}

pub fn run_with_motion(
    app: impl Application + 'static,
    options: Options,
    preference: MotionPreference,
) -> Result<(), Error> {
    let event_loop = EventLoop::new().map_err(err)?;
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut host = Host {
        app,
        options,
        state: None,
        layout: Layout::default(),
        scene: Scene::default(),
        base_scene: Scene::default(),
        presented: PresentedScene::default(),
        motion: MotionState::default(),
        motion_frame: MotionFrame::default(),
        preference,
        reduced: preference.is_reduced(),
        started: Instant::now(),
        last_view: None,
        window_focused: true,
        occluded: false,
        revealed_scroll: std::collections::BTreeSet::new(),
        timers: timing::TimingState::default(),
        input: Input::default(),
        modifiers: ModifiersState::empty(),
        system_dark: false,
        error: None,
        ime_composing: false,
    };
    event_loop.run_app(&mut host).map_err(err)?;
    match host.error {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

/// Render a verified native frame to PNG, using exactly the desktop renderer.
/// Creates a new file only; existing captures are never silently overwritten.
pub fn capture(
    app: &impl Application,
    path: &Path,
    width: u32,
    height: u32,
    dark: bool,
) -> Result<String, Error> {
    let mut session = CaptureSession::new()?;
    session.write(app.view(), path, width, height, dark)?;
    Ok(session.adapter_name().into())
}

/// Capture one deterministic native motion timestamp using the live renderer.
#[allow(clippy::too_many_arguments)]
pub fn capture_at(
    app: &impl Application,
    from: &View,
    path: &Path,
    width: u32,
    height: u32,
    dark: bool,
    now_ms: u64,
    preference: MotionPreference,
) -> Result<String, Error> {
    let mut session = CaptureSession::new()?;
    session.write_at(app, from, path, width, height, dark, now_ms, preference)?;
    Ok(session.adapter_name().into())
}

/// Reuses the real renderer across a batch of native documentation captures.
pub struct CaptureSession {
    renderer: Renderer,
}
impl CaptureSession {
    pub fn new() -> Result<Self, Error> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let renderer = pollster::block_on(Renderer::new(
            &instance,
            None,
            wgpu::TextureFormat::Rgba8UnormSrgb,
        ))?;
        Ok(Self { renderer })
    }
    pub fn adapter_name(&self) -> &str {
        &self.renderer.adapter_name
    }
    /// Write a new PNG from validated native view data. Never overwrite files.
    pub fn write(
        &mut self,
        view: &View,
        path: &Path,
        width: u32,
        height: u32,
        dark: bool,
    ) -> Result<(), Error> {
        if width == 0 || height == 0 || width > 8192 || height > 8192 {
            return Err("capture dimensions must be 1..8192".into());
        }
        let scene = Layout::default().compute(
            view,
            width as f32,
            height as f32,
            &mut self.renderer.text,
        )?;
        let theme = if dark { Theme::dark() } else { Theme::light() };
        let frame = self.renderer.prepare(
            view,
            &scene,
            &Input::default(),
            &theme,
            dark,
            1.0,
            (width as f32, height as f32),
        )?;
        self.write_frame(frame, path, width, height)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn write_at(
        &mut self,
        app: &impl Application,
        from: &View,
        path: &Path,
        width: u32,
        height: u32,
        dark: bool,
        now_ms: u64,
        preference: MotionPreference,
    ) -> Result<(), Error> {
        if width == 0 || height == 0 || width > 8192 || height > 8192 {
            return Err("capture dimensions must be 1..8192".into());
        }
        let mut layout = Layout::default();
        let previous_scene =
            layout.compute(from, width as f32, height as f32, &mut self.renderer.text)?;
        let current_view = app.view();
        let scene = layout.compute(
            current_view,
            width as f32,
            height as f32,
            &mut self.renderer.text,
        )?;
        let input = Input::default();
        let reduced = preference.is_reduced();
        let mut motion = MotionState::default();
        motion.commit(
            &motion::targets(from, &previous_scene, &input, None),
            0,
            reduced,
        )?;
        motion.commit(
            &motion::targets(current_view, &scene, &input, Some((from, &previous_scene))),
            0,
            reduced,
        )?;
        let motion_frame = motion.sample(now_ms, reduced);
        let presented = present(current_view, &scene, &motion_frame);
        let theme = if dark { Theme::dark() } else { Theme::light() };
        let frame = self.renderer.prepare_presented(
            current_view,
            &presented,
            &input,
            &theme,
            dark,
            1.0,
            (width as f32, height as f32),
        )?;
        self.write_frame(frame, path, width, height)
    }

    fn write_frame(
        &mut self,
        frame: Frame,
        path: &Path,
        width: u32,
        height: u32,
    ) -> Result<(), Error> {
        let texture = self
            .renderer
            .device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("Ubi capture"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            });
        self.renderer
            .draw(&texture.create_view(&Default::default()), &frame);
        let rgba = self.renderer.read_rgba(&texture, width, height)?;
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(err)?;
        let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().map_err(err)?;
        writer.write_image_data(&rgba).map_err(err)?;
        writer.finish().map_err(err)?;
        Ok(())
    }
}
fn err(e: impl std::fmt::Display) -> Error {
    Error(e.to_string())
}
struct SurfaceState {
    instance: wgpu::Instance,
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    renderer: Renderer,
}
impl SurfaceState {
    fn new(window: Arc<Window>) -> Result<Self, Error> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let surface = instance.create_surface(window.clone()).map_err(err)?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&surface),
            ..Default::default()
        }))
        .map_err(err)?;
        let capabilities = surface.get_capabilities(&adapter);
        let format = capabilities
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .or(capabilities.formats.first().copied())
            .ok_or("surface has no supported format")?;
        let renderer = pollster::block_on(Renderer::with_adapter(&adapter, format))?;
        let size = window.inner_size();
        let mut config = surface
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
            .ok_or("surface unavailable")?;
        config.format = format;
        config.present_mode = wgpu::PresentMode::AutoVsync;
        surface.configure(&renderer.device, &config);
        window.set_ime_allowed(true);
        Ok(Self {
            instance,
            window,
            surface,
            config,
            renderer,
        })
    }
    fn resize(&mut self) {
        let size = self.window.inner_size();
        if size.width > 0 && size.height > 0 {
            self.config.width = size.width;
            self.config.height = size.height;
            self.surface.configure(&self.renderer.device, &self.config);
        }
    }
    fn draw(
        &mut self,
        view: &View,
        presented: &PresentedScene,
        input: &Input,
        dark: bool,
    ) -> Result<bool, Error> {
        let size = self.window.inner_size();
        if size.width == 0 || size.height == 0 {
            return Ok(false);
        }
        let dpi = self.window.scale_factor() as f32;
        let w = size.width as f32 / dpi;
        let h = size.height as f32 / dpi;
        let texture = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t)
            | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            wgpu::CurrentSurfaceTexture::Outdated => {
                self.resize();
                self.window.request_redraw();
                return Ok(false);
            }
            wgpu::CurrentSurfaceTexture::Lost => {
                self.surface = self
                    .instance
                    .create_surface(self.window.clone())
                    .map_err(err)?;
                self.resize();
                self.window.request_redraw();
                return Ok(false);
            }
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                return Ok(false)
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                return Err("GPU surface validation failure".into())
            }
        };
        let frame = self.renderer.prepare_presented(
            view,
            presented,
            input,
            &Theme::light(),
            dark,
            dpi,
            (w, h),
        )?;
        self.renderer
            .draw(&texture.texture.create_view(&Default::default()), &frame);
        self.window.pre_present_notify();
        self.renderer.queue.present(texture);
        Ok(true)
    }
}
struct Host<A> {
    app: A,
    options: Options,
    state: Option<SurfaceState>,
    layout: Layout,
    base_scene: Scene,
    scene: Scene,
    presented: PresentedScene,
    motion: MotionState,
    motion_frame: MotionFrame,
    preference: MotionPreference,
    reduced: bool,
    started: Instant,
    last_view: Option<View>,
    window_focused: bool,
    occluded: bool,
    revealed_scroll: std::collections::BTreeSet<String>,
    timers: timing::TimingState,
    input: Input,
    modifiers: ModifiersState,
    system_dark: bool,
    error: Option<Error>,
    ime_composing: bool,
}
impl<A: Application> Host<A> {
    fn fail(&mut self, e: Error, event_loop: &ActiveEventLoop) {
        self.error = Some(e);
        event_loop.exit();
    }
    fn dispatch(&mut self, event: Option<Event>) {
        if let Some(event) = event {
            match self.app.dispatch(&event) {
                Ok(changes) => {
                    let changed = !changes.is_empty();
                    for key in changes.unmounted {
                        if self.input.focused.as_ref() == Some(&key) {
                            self.input.focused = None;
                        }
                        if self.input.captured.as_ref() == Some(&key) {
                            self.input.captured = None;
                        }
                    }
                    self.input.sync(self.app.view());
                    // OS input events may already be queued before redraw.
                    // Refresh hit geometry as part of committing a changed view.
                    if changed {
                        if let Err(e) = self.refresh_scene() {
                            self.error = Some(e);
                        }
                    }
                }
                Err(e) => eprintln!("Ubi UI event rejected (state preserved): {e}"),
            }
        }
    }
    fn text_input(&mut self, text: &str) {
        let view = self.app.view().clone();
        let menu_focus = self
            .input
            .focused
            .as_deref()
            .and_then(|key| view.nodes.get(key))
            .is_some_and(|node| {
                matches!(
                    node.kind,
                    ubi_ui_core::Kind::MenuItem | ubi_ui_core::Kind::OptionItem
                )
            });
        if menu_focus {
            self.input.typeahead(&view, text, self.now_ms());
        } else {
            let event = self.input.text(&view, text);
            self.dispatch(event);
        }
    }
    fn now_ms(&self) -> u64 {
        self.started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64
    }
    fn visible(&self) -> bool {
        self.state.is_some() && !self.occluded
    }
    fn active(&self) -> bool {
        self.visible() && self.window_focused
    }
    fn sync_timers(&mut self) {
        let now = self.now_ms();
        let view = self.app.view().clone();
        self.timers.sync(
            &view,
            &self.scene,
            &self.input,
            now,
            self.active(),
            self.reduced,
        );
        // Poll only while the native window can receive input. Inactive Toast
        // and rotation timers are paused by sync; avoiding a poll also keeps a
        // Tooltip request from being consumed while the host is occluded.
        if !self.active() {
            return;
        }
        for event in self.timers.poll(&view, now) {
            let current = self.app.view().clone();
            if timer_event_current(
                &event,
                &current,
                &self.scene,
                &self.input,
                self.active(),
                self.reduced,
            ) {
                self.dispatch(Some(event));
            }
        }
    }
    fn ambient_skeleton_visible(&self) -> bool {
        self.visible()
            && !self.reduced
            && self.scene.order.iter().any(|key| {
                self.app.view().nodes.get(key).is_some_and(|node| {
                    node.kind == ubi_ui_core::Kind::Skeleton
                        && !node.motion.is_some_and(|motion| motion.duration_ms == 0)
                }) && self.scene.boxes.get(key).is_some_and(|layout| {
                    let visible = layout
                        .bounds
                        .intersect(layout.clip)
                        .intersect(self.scene.viewport);
                    visible.width > 0.0 && visible.height > 0.0
                })
            })
    }
    fn refresh_scene(&mut self) -> Result<(), Error> {
        let Some(state) = &mut self.state else {
            return Ok(());
        };
        let size = state.window.inner_size();
        let dpi = state.window.scale_factor() as f32;
        if size.width == 0 || size.height == 0 {
            return Ok(());
        }
        self.reduced = self.preference.is_reduced();
        let view = self.app.view().clone();
        let scene = self.layout.compute(
            &view,
            size.width as f32 / dpi,
            size.height as f32 / dpi,
            &mut state.renderer.text,
        )?;
        let previous = self.last_view.as_ref().map(|view| (view, &self.scene));
        let now_ms = self.now_ms();
        let targets = motion::targets_at(
            &view,
            &scene,
            &self.input,
            previous,
            &mut self.revealed_scroll,
            now_ms,
            self.reduced,
        );
        self.motion.commit(&targets, self.now_ms(), self.reduced)?;
        self.motion_frame = self.motion.sample(self.now_ms(), self.reduced);
        self.presented = present(&view, &scene, &self.motion_frame);
        self.base_scene = scene;
        self.scene = self.presented.scene.clone();
        self.last_view = Some(view);
        Ok(())
    }
}
impl<A: Application> ApplicationHandler for Host<A> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }
        let attributes = Window::default_attributes()
            .with_title(&self.options.title)
            .with_inner_size(LogicalSize::new(self.options.width, self.options.height))
            .with_min_inner_size(LogicalSize::new(360, 300))
            .with_visible(!self.options.smoke);
        let result = event_loop
            .create_window(attributes)
            .map_err(err)
            .and_then(|w| SurfaceState::new(Arc::new(w)));
        match result {
            Ok(state) => {
                self.state = Some(state);
                self.window_focused = true;
                self.occluded = false;
                self.system_dark =
                    self.state.as_ref().unwrap().window.theme() == Some(winit::window::Theme::Dark);
                if let Err(e) = self.refresh_scene() {
                    self.fail(e, event_loop);
                    return;
                }
                if self.options.smoke {
                    let result = self.state.as_mut().unwrap().draw(
                        self.app.view(),
                        &self.presented,
                        &self.input,
                        self.system_dark,
                    );
                    match result {
                        Ok(true) => {
                            println!(
                                "UBI_NATIVE_UI_SMOKE_OK adapter={} nodes={}",
                                self.state.as_ref().unwrap().renderer.adapter_name,
                                self.app.view().nodes.len()
                            );
                            event_loop.exit();
                        }
                        Ok(false) => {
                            self.fail("native smoke did not acquire a frame".into(), event_loop);
                        }
                        Err(e) => self.fail(e, event_loop),
                    }
                } else {
                    self.state.as_ref().unwrap().window.request_redraw();
                }
            }
            Err(e) => self.fail(e, event_loop),
        }
    }
    fn suspended(&mut self, _: &ActiveEventLoop) {
        self.input.cancel();
        self.window_focused = false;
        self.occluded = true;
        self.state = None;
        self.sync_timers();
    }
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let reduced = self.preference.is_reduced();
        if reduced != self.reduced {
            self.reduced = reduced;
            if let Err(e) = self.refresh_scene() {
                self.error = Some(e);
            }
        }
        if self.error.is_some() {
            event_loop.exit();
            return;
        }
        if !self.visible() {
            event_loop.set_control_flow(ControlFlow::Wait);
            return;
        }
        self.sync_timers();
        let now = self.now_ms();
        let ambient_wake = self
            .ambient_skeleton_visible()
            .then_some(now.saturating_add(32));
        let timer_wake = self.active().then(|| self.timers.next_wake_ms()).flatten();
        let deadline_ms = [self.motion_frame.next_wake_ms, timer_wake, ambient_wake]
            .into_iter()
            .flatten()
            .min();
        if let Some(deadline_ms) = deadline_ms {
            let deadline = self.started + Duration::from_millis(deadline_ms);
            if deadline <= Instant::now() {
                if let Some(state) = &self.state {
                    state.window.request_redraw();
                }
                event_loop.set_control_flow(ControlFlow::Wait);
            } else {
                event_loop.set_control_flow(ControlFlow::WaitUntil(deadline));
            }
        } else {
            event_loop.set_control_flow(ControlFlow::Wait);
        }
    }
    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        if self.state.as_ref().is_none_or(|s| s.window.id() != id) {
            return;
        }
        if let WindowEvent::RedrawRequested = event {
            if let Err(e) = self.refresh_scene() {
                self.fail(e, event_loop);
                return;
            }
            let result = self.state.as_mut().unwrap().draw(
                self.app.view(),
                &self.presented,
                &self.input,
                self.system_dark,
            );
            if let Err(e) = result {
                self.fail(e, event_loop);
            }
            return;
        }
        let previous_focus = self.input.focused.clone();
        let mut redraw = true;
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
                return;
            }
            WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
                self.state.as_mut().unwrap().resize();
                if let Err(e) = self.refresh_scene() {
                    self.fail(e, event_loop);
                    return;
                }
            }
            WindowEvent::ThemeChanged(theme) => {
                self.system_dark = theme == winit::window::Theme::Dark
            }
            WindowEvent::Focused(false) => {
                self.window_focused = false;
                self.input.cancel();
                redraw = false;
            }
            WindowEvent::Focused(true) => {
                self.window_focused = true;
                if let Err(e) = self.refresh_scene() {
                    self.fail(e, event_loop);
                    return;
                }
            }
            WindowEvent::Occluded(true) => {
                self.occluded = true;
                redraw = false;
            }
            WindowEvent::Occluded(false) => {
                self.occluded = false;
                if let Err(e) = self.refresh_scene() {
                    self.fail(e, event_loop);
                    return;
                }
            }
            WindowEvent::CursorLeft { .. } => self.input.pointer_leave(),
            WindowEvent::CursorMoved { position, .. } => {
                let hovered = self.input.hovered.clone();
                let dpi = self.state.as_ref().unwrap().window.scale_factor();
                let event = self.input.pointer_move(
                    self.app.view(),
                    &self.scene,
                    (position.x / dpi) as f32,
                    (position.y / dpi) as f32,
                );
                redraw = hovered != self.input.hovered || event.is_some();
                self.dispatch(event);
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if button == MouseButton::Right && state == ElementState::Pressed {
                    let point = self.input.pointer;
                    if let Some((menu, event)) = self.input.context_request_at(
                        self.app.view(),
                        &self.scene,
                        point.0,
                        point.1,
                    ) {
                        self.layout.set_context_point(&menu, Some(point));
                        self.dispatch(Some(event));
                    }
                } else if button == MouseButton::Left {
                    let event = if state == ElementState::Pressed {
                        self.input.pointer_down(self.app.view(), &self.scene)
                    } else {
                        self.input.pointer_up(self.app.view(), &self.scene)
                    };
                    self.dispatch(event);
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let dy = match delta {
                    MouseScrollDelta::LineDelta(_, y) => -y * 32.0,
                    MouseScrollDelta::PixelDelta(p) => {
                        (-p.y / self.state.as_ref().unwrap().window.scale_factor()) as f32
                    }
                };
                redraw = self.layout.scroll_at(
                    self.app.view(),
                    &self.scene,
                    self.input.pointer.0,
                    self.input.pointer.1,
                    dy,
                );
                if redraw {
                    if let Err(e) = self.refresh_scene() {
                        self.fail(e, event_loop);
                        return;
                    }
                }
            }
            WindowEvent::ModifiersChanged(m) => {
                self.modifiers = m.state();
                redraw = false;
            }
            WindowEvent::Ime(Ime::Preedit(text, _)) => {
                self.ime_composing = !text.is_empty();
                redraw = false;
            }
            WindowEvent::Ime(Ime::Commit(text)) => {
                self.ime_composing = false;
                self.text_input(&text);
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let down = event.state == ElementState::Pressed;
                let command = self.modifiers.control_key() || self.modifiers.super_key();
                if down
                    && self.modifiers.shift_key()
                    && event.logical_key == Key::Named(NamedKey::F10)
                {
                    if let Some((menu, event)) = self.input.context_request_focused(self.app.view())
                    {
                        self.layout.set_context_point(&menu, None);
                        self.dispatch(Some(event));
                    }
                } else if self.ime_composing && event.logical_key == Key::Named(NamedKey::Enter) {
                    redraw = false;
                } else if down
                    && command
                    && matches!(&event.logical_key,Key::Character(c)if c.eq_ignore_ascii_case("a"))
                {
                    self.input.select_all = true;
                } else if let Some(key) = map_key(&event.logical_key) {
                    if !event.repeat || !matches!(key, UiKey::Enter | UiKey::Space) {
                        let text_layout = self.input.focused.as_ref().and_then(|focused| {
                            let n = self.app.view().nodes.get(focused)?;
                            if !down
                                || n.kind != ubi_ui_core::Kind::TextArea
                                || !matches!(
                                    key,
                                    UiKey::Up | UiKey::Down | UiKey::Home | UiKey::End
                                )
                            {
                                return None;
                            }
                            let bounds = self.scene.boxes.get(focused)?.bounds;
                            let state = self.state.as_mut()?;
                            let dpi = state.window.scale_factor() as f32;
                            Some(state.renderer.text.caret_layout(
                                &n.value,
                                n.style.font_size * dpi,
                                (bounds.width - 24.0).max(1.0) * dpi,
                            ))
                        });
                        let e = self.input.key_with_text_layout(
                            self.app.view(),
                            key,
                            down,
                            self.modifiers.shift_key(),
                            text_layout.as_ref(),
                        );
                        self.dispatch(e);
                    }
                    if key == UiKey::Space && down && !command && !self.ime_composing {
                        let e = self.input.text(self.app.view(), " ");
                        self.dispatch(e);
                    }
                } else if down && !command && !self.ime_composing {
                    if let Some(text) = event.text {
                        self.text_input(&text);
                    }
                }
            }
            _ => redraw = false,
        }
        if previous_focus != self.input.focused {
            if let Some(key) = self.input.focused.clone() {
                if self.layout.reveal(self.app.view(), &self.scene, &key) {
                    if let Err(e) = self.refresh_scene() {
                        self.fail(e, event_loop);
                        return;
                    }
                    redraw = true;
                }
            }
        }
        self.sync_timers();
        if redraw {
            if let Err(e) = self.refresh_scene() {
                self.fail(e, event_loop);
                return;
            }
            let state = self.state.as_ref().unwrap();
            state.window.request_redraw();
            let cursor = if self.input.hovered.is_some() {
                winit::window::CursorIcon::Pointer
            } else {
                winit::window::CursorIcon::Default
            };
            state.window.set_cursor(cursor);
        }
    }
}

fn timer_event_current(
    event: &Event,
    view: &View,
    scene: &Scene,
    input: &Input,
    host_active: bool,
    reduced: bool,
) -> bool {
    if !host_active || event.value.get("tag").and_then(|value| value.as_str()) != Some("Activate") {
        return false;
    }
    let Some(message) = event
        .value
        .get("values")
        .and_then(|values| values.as_array())
        .and_then(|values| values.first())
    else {
        return false;
    };
    let Some(node) = view.nodes.get(&event.key) else {
        return false;
    };
    match (node.kind, node.overlay_kind, node.effect) {
        (Kind::Overlay, Some(OverlayKind::Toast), _) => {
            node.open
                && node.timeout_ms > 0
                && node.dismiss.as_ref() == Some(message)
                && scene_has_visible_node(scene, &event.key)
                && !pointer_over_subtree(view, scene, input, &event.key)
                && !focus_within(view, scene, input, &event.key)
        }
        (_, _, Some(Effect::RotatingText)) => {
            node.auto_play
                && !node.paused
                && !reduced
                && node.advance.as_ref() == Some(message)
                && scene_has_visible_node(scene, &event.key)
                && !pointer_over_subtree(view, scene, input, &event.key)
                && !focus_within(view, scene, input, &event.key)
        }
        (Kind::Overlay, Some(OverlayKind::Tooltip), _) => {
            let Some(anchor) = node.anchor.as_deref() else {
                return false;
            };
            if node.open {
                node.dismiss.as_ref() == Some(message)
                    && !pointer_over_subtree(view, scene, input, anchor)
                    && !pointer_over_subtree(view, scene, input, &event.key)
                    && !focus_within(view, scene, input, anchor)
                    && !focus_within(view, scene, input, &event.key)
            } else {
                node.open_request.as_ref() == Some(message)
                    && (pointer_over_subtree(view, scene, input, anchor)
                        || focus_within(view, scene, input, anchor))
            }
        }
        _ => false,
    }
}

fn scene_has_visible_node(scene: &Scene, key: &str) -> bool {
    scene.order.iter().any(|visible| visible == key)
        && scene.boxes.get(key).is_some_and(|layout| {
            let bounds = layout
                .bounds
                .intersect(layout.clip)
                .intersect(scene.viewport);
            bounds.x.is_finite()
                && bounds.y.is_finite()
                && bounds.width.is_finite()
                && bounds.height.is_finite()
                && bounds.width > 0.0
                && bounds.height > 0.0
        })
}

fn pointer_over_subtree(view: &View, scene: &Scene, input: &Input, root: &str) -> bool {
    if !input.pointer_inside {
        return false;
    }
    if input.hovered.as_deref().is_some_and(|hovered| {
        scene_has_visible_node(scene, hovered) && is_in_subtree(view, root, hovered)
    }) {
        return true;
    }
    scene_has_visible_node(scene, root)
        && scene.boxes[root]
            .bounds
            .intersect(scene.boxes[root].clip)
            .contains(input.pointer.0, input.pointer.1)
}

fn focus_within(view: &View, scene: &Scene, input: &Input, root: &str) -> bool {
    input.focused.as_deref().is_some_and(|focused| {
        scene_has_visible_node(scene, focused) && is_in_subtree(view, root, focused)
    })
}

fn is_in_subtree(view: &View, root: &str, candidate: &str) -> bool {
    let mut pending = vec![root.to_owned()];
    let mut visited = std::collections::BTreeSet::new();
    while let Some(key) = pending.pop() {
        if !visited.insert(key.clone()) {
            continue;
        }
        if key == candidate {
            return true;
        }
        if let Some(node) = view.nodes.get(&key) {
            pending.extend(node.children.iter().cloned());
        }
        if visited.len() >= 4096 {
            break;
        }
    }
    false
}

fn map_key(key: &Key) -> Option<UiKey> {
    Some(match key {
        Key::Named(NamedKey::Tab) => UiKey::Tab,
        Key::Named(NamedKey::Enter) => UiKey::Enter,
        Key::Named(NamedKey::Space) => UiKey::Space,
        Key::Named(NamedKey::Escape) => UiKey::Escape,
        Key::Named(NamedKey::Backspace) => UiKey::Backspace,
        Key::Named(NamedKey::Delete) => UiKey::Delete,
        Key::Named(NamedKey::ArrowLeft) => UiKey::Left,
        Key::Named(NamedKey::ArrowRight) => UiKey::Right,
        Key::Named(NamedKey::ArrowUp) => UiKey::Up,
        Key::Named(NamedKey::ArrowDown) => UiKey::Down,
        Key::Named(NamedKey::Home) => UiKey::Home,
        Key::Named(NamedKey::End) => UiKey::End,
        _ => return None,
    })
}
