use serde_json::json;
use ubi_ui_core::View;
use ubi_ui_layout::{Layout, TextMeasure};
use ubi_ui_render::{Renderer, Text};
use ubi_ui_theme::Theme;

#[test]
fn text_measurement_wraps_and_reuses_cache() {
    let mut text = Text::new().unwrap();
    let wide = text.measure("Native Ubi text with words", 14.0, 500.0);
    let narrow = text.measure("Native Ubi text with words", 14.0, 80.0);
    assert!(narrow.1 > wide.1);
    let count = text.cache_len();
    assert_eq!(
        text.measure("Native Ubi text with words", 14.0, 80.0),
        narrow
    );
    assert_eq!(text.cache_len(), count);
}

#[test]
fn gpu_readback_contains_rounded_button_text_and_cached_glyphs() {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let mut renderer = pollster::block_on(Renderer::new(
        &instance,
        None,
        wgpu::TextureFormat::Rgba8UnormSrgb,
    ))
    .unwrap();
    let view=View::decode(json!({"schemaVersion":1,"root":"root","nodes":[
        {"key":"root","kind":{"tag":"Column","values":[]},"props":[{"tag":"Padding","values":[16]}],"children":["button"]},
        {"key":"button","kind":{"tag":"Button","values":[]},"props":[{"tag":"Content","values":["Ubi native"]},{"tag":"Width","values":[160]}],"children":[]}
    ]})).unwrap();
    let scene = Layout::default()
        .compute(&view, 256.0, 128.0, &mut renderer.text)
        .unwrap();
    let target = renderer.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Ubi test"),
        size: wgpu::Extent3d {
            width: 256,
            height: 128,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let frame = renderer
        .prepare(
            &view,
            &scene,
            &Default::default(),
            &Theme::light(),
            false,
            1.0,
            (256.0, 128.0),
        )
        .unwrap();
    assert!(frame.quad_count() > 5);
    renderer.draw(&target.create_view(&Default::default()), &frame);
    let rgba = renderer.read_rgba(&target, 256, 128).unwrap();
    let pixel = |x: usize, y: usize| &rgba[(y * 256 + x) * 4..(y * 256 + x) * 4 + 4];
    assert_eq!(pixel(0, 0), [244, 245, 247, 255]);
    let blue = pixel(30, 30);
    assert!(blue[2] > 150 && blue[0] < 100, "{blue:?}");
    assert_ne!(pixel(16, 16), pixel(30, 30));
    let white = rgba
        .chunks_exact(4)
        .filter(|p| p[0] > 230 && p[1] > 230 && p[2] > 230)
        .count();
    assert!(white > 100);
    let glyphs = renderer.cached_glyphs();
    renderer
        .prepare(
            &view,
            &scene,
            &Default::default(),
            &Theme::light(),
            false,
            1.0,
            (256.0, 128.0),
        )
        .unwrap();
    assert_eq!(renderer.cached_glyphs(), glyphs);
    eprintln!("GPU verified: {}", renderer.adapter_name);
}
