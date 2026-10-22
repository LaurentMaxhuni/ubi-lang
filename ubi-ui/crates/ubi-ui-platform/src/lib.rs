//! winit desktop host. Component, layout and rendering logic stay platform-neutral.
use std::{path::Path, sync::Arc};
pub use ubi_ui_core::{Changes, Error, Tree, View};
pub use ubi_ui_input::Event;
use ubi_ui_input::{Input, Key as UiKey};
use ubi_ui_layout::{Layout, Scene};
use ubi_ui_render::Renderer;
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
    let event_loop = EventLoop::new().map_err(err)?;
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut host = Host {
        app,
        options,
        state: None,
        layout: Layout::default(),
        scene: Scene::default(),
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
    if width == 0 || height == 0 || width > 8192 || height > 8192 {
        return Err("capture dimensions must be 1..8192".into());
    }
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let format = wgpu::TextureFormat::Rgba8UnormSrgb;
    let mut renderer = pollster::block_on(Renderer::new(&instance, None, format))?;
    let scene =
        Layout::default().compute(app.view(), width as f32, height as f32, &mut renderer.text)?;
    let frame = renderer.prepare(
        app.view(),
        &scene,
        &Input::default(),
        &Theme::light(),
        dark,
        1.0,
        (width as f32, height as f32),
    )?;
    let texture = renderer.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Ubi capture"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    renderer.draw(&texture.create_view(&Default::default()), &frame);
    let rgba = renderer.read_rgba(&texture, width, height)?;
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
    Ok(renderer.adapter_name)
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
        layout: &mut Layout,
        scene: &mut Scene,
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
        *scene = layout.compute(view, w, h, &mut self.renderer.text)?;
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
        let frame =
            self.renderer
                .prepare(view, scene, input, &Theme::light(), dark, dpi, (w, h))?;
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
    scene: Scene,
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
                    for key in changes.unmounted {
                        if self.input.focused.as_ref() == Some(&key) {
                            self.input.focused = None;
                        }
                        if self.input.captured.as_ref() == Some(&key) {
                            self.input.captured = None;
                        }
                    }
                    self.input.sync(self.app.view());
                }
                Err(e) => eprintln!("Ubi UI event rejected (state preserved): {e}"),
            }
        }
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
            Ok(mut state) => {
                self.system_dark = state.window.theme() == Some(winit::window::Theme::Dark);
                if self.options.smoke {
                    match state.draw(
                        self.app.view(),
                        &mut self.layout,
                        &mut self.scene,
                        &self.input,
                        self.system_dark,
                    ) {
                        Ok(true) => {
                            println!(
                                "UBI_NATIVE_UI_SMOKE_OK adapter={} nodes={}",
                                state.renderer.adapter_name,
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
                    state.window.request_redraw();
                }
                self.state = Some(state);
            }
            Err(e) => self.fail(e, event_loop),
        }
    }
    fn suspended(&mut self, _: &ActiveEventLoop) {
        self.input.cancel();
        self.state = None;
    }
    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        if self.state.as_ref().is_none_or(|s| s.window.id() != id) {
            return;
        }
        if let WindowEvent::RedrawRequested = event {
            let result = self.state.as_mut().unwrap().draw(
                self.app.view(),
                &mut self.layout,
                &mut self.scene,
                &self.input,
                self.system_dark,
            );
            if let Err(e) = result {
                self.fail(e, event_loop);
            }
            return;
        }
        let mut redraw = true;
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
                return;
            }
            WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
                self.state.as_mut().unwrap().resize()
            }
            WindowEvent::ThemeChanged(theme) => {
                self.system_dark = theme == winit::window::Theme::Dark
            }
            WindowEvent::Focused(false) => self.input.cancel(),
            WindowEvent::CursorLeft { .. } => self.input.hovered = None,
            WindowEvent::CursorMoved { position, .. } => {
                let dpi = self.state.as_ref().unwrap().window.scale_factor();
                let event = self.input.pointer_move(
                    self.app.view(),
                    &self.scene,
                    (position.x / dpi) as f32,
                    (position.y / dpi) as f32,
                );
                self.dispatch(event);
            }
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => {
                let event = if state == ElementState::Pressed {
                    self.input.pointer_down(self.app.view(), &self.scene)
                } else {
                    self.input.pointer_up(self.app.view(), &self.scene)
                };
                self.dispatch(event);
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
                let event = self.input.text(self.app.view(), &text);
                self.dispatch(event);
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let down = event.state == ElementState::Pressed;
                let command = self.modifiers.control_key() || self.modifiers.super_key();
                if down
                    && command
                    && matches!(&event.logical_key,Key::Character(c)if c.eq_ignore_ascii_case("a"))
                {
                    self.input.select_all = true;
                } else if let Some(key) = map_key(&event.logical_key) {
                    if !event.repeat || !matches!(key, UiKey::Enter | UiKey::Space) {
                        let e =
                            self.input
                                .key(self.app.view(), key, down, self.modifiers.shift_key());
                        self.dispatch(e);
                    }
                    if key == UiKey::Space && down && !command && !self.ime_composing {
                        let e = self.input.text(self.app.view(), " ");
                        self.dispatch(e);
                    }
                } else if down && !command && !self.ime_composing {
                    if let Some(text) = event.text {
                        let e = self.input.text(self.app.view(), &text);
                        self.dispatch(e);
                    }
                }
            }
            _ => redraw = false,
        }
        if redraw {
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
        Key::Named(NamedKey::Home) => UiKey::Home,
        Key::Named(NamedKey::End) => UiKey::End,
        _ => return None,
    })
}
