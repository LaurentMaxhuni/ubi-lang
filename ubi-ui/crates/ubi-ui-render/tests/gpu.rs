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

#[test]
fn native_radio_state_and_multiline_text_render_to_distinct_pixels() {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let mut renderer = pollster::block_on(Renderer::new(
        &instance,
        None,
        wgpu::TextureFormat::Rgba8UnormSrgb,
    ))
    .unwrap();
    let make = |key: &str, kind: &str, props: serde_json::Value, children: Vec<&str>| json!({"key":key,"kind":{"tag":kind,"values":[]},"props":props,"children":children});
    let mut view=View::decode(json!({"schemaVersion":1,"root":"root","nodes":[
        make("root","Column",json!([{"tag":"Padding","values":[16]}]),vec!["radio","area","avatar","kbd"]),
        make("radio","Radio",json!([{"tag":"Content","values":["Weekly"]},{"tag":"Selected","values":[true]}]),vec![]),
        make("area","TextArea",json!([{"tag":"Value","values":["First line\nSecond line"]},{"tag":"Width","values":[280]}]),vec![]),
        make("avatar","Avatar",json!([{"tag":"Content","values":["AL"]},{"tag":"Label","values":["Alex"]}]),vec![]),
        make("kbd","Kbd",json!([{"tag":"Content","values":["⌘ K"]}]),vec![])
    ]})).unwrap();
    let scene = Layout::default()
        .compute(&view, 320.0, 320.0, &mut renderer.text)
        .unwrap();
    assert_eq!(scene.boxes["avatar"].bounds.width, 40.0);
    assert_eq!(scene.boxes["avatar"].bounds.height, 40.0);
    assert!(scene.boxes["kbd"].bounds.width < 100.0);
    let texture = renderer.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("new native controls"),
        size: wgpu::Extent3d {
            width: 320,
            height: 320,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let mut frames = Vec::new();
    for selected in [true, false] {
        view.nodes.get_mut("radio").unwrap().checked = selected;
        let frame = renderer
            .prepare(
                &view,
                &scene,
                &Default::default(),
                &Theme::light(),
                false,
                1.0,
                (320.0, 320.0),
            )
            .unwrap();
        renderer.draw(&texture.create_view(&Default::default()), &frame);
        frames.push(renderer.read_rgba(&texture, 320, 320).unwrap());
    }
    let radio = scene.boxes["radio"].bounds;
    let pixel = ((radio.y + radio.height * 0.5) as usize * 320 + (radio.x + 12.0) as usize) * 4;
    assert_ne!(&frames[0][pixel..pixel + 4], &frames[1][pixel..pixel + 4]);
    let area = scene.boxes["area"].bounds;
    let ink_rows = (area.y as usize + 8..area.y as usize + 80)
        .filter(|y| {
            (area.x as usize + 12..area.x as usize + 130).any(|x| {
                let p = (*y * 320 + x) * 4;
                frames[0][p] < 150
            })
        })
        .count();
    assert!(
        ink_rows > 15,
        "multiline ink must occupy multiple text rows, got {ink_rows}"
    );
}
