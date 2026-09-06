use sophia_engine::*;
use sophia_protocol::*;
use sophia_renderer_live::*;
#[path = "../../sophia-protocol/tests/support/reference_fixture.rs"]
mod fixture;
#[test]
fn reference_sheet_uses_bundled_text_and_composites_translucent_background() {
    let output = HeadlessOutput {
        id: OutputId::from_raw(1),
        size: Size {
            width: 1280,
            height: 720,
        },
        scale: 1,
    };
    let bounds = Rect {
        x: 0,
        y: 0,
        width: 1280,
        height: 720,
    };
    let mut cache = CompositorTextRasterCache::default();
    let catalog = fixture::catalog(36);
    let c = fixture::candidate(36);
    let (projection, _, _) = reference_sheet_projection(&c, &catalog, 1, bounds, |text, size| {
        cache.measure(text, size)
    })
    .unwrap();
    let mut commands = vec![CompositorDisplayCommand::Rect(CompositorRect {
        node: CompositorNodeId::DescriptorOverlay {
            projection: 2,
            slot: 0,
            role: DescriptorOverlayNodeRole::Panel,
        },
        generation: 1,
        geometry: bounds,
        color: CompositorRgb8 {
            red: 100,
            green: 100,
            blue: 100,
        },
        opacity: 255,
    })];
    commands.extend(projection.commands);
    let mut scene = LiveProductionCpuScene::new(output.size);
    let report = scene
        .compose_display_list(
            output,
            &[],
            &CompositorDisplayList {
                output: output.id,
                commands,
            },
            None,
        )
        .unwrap();
    let pixel = |x: i32, y: i32| {
        let i = (y as usize * report.frame.stride as usize) + (x as usize * 4);
        &report.frame.bytes[i..i + 4]
    };
    assert_eq!(pixel(0, 0), &[100, 100, 100, 255]);
    let g = projection.geometry;
    // Triad fb8fb27 with the same font, rows and 1280x720 output.
    assert_eq!(
        g,
        Rect {
            x: 404,
            y: 52,
            width: 472,
            height: 615
        }
    );
    assert_eq!(pixel(g.x, g.y), &[255, 168, 98, 255]);
    assert_eq!(pixel(g.x + 6, g.y + 6), &[34, 29, 28, 255]);
    assert_eq!(report.layers_composed, report.layers_input);
    if let Ok(path) = std::env::var("SOPHIA_REFERENCE_PREVIEW") {
        use std::io::Write;
        let mut file = std::fs::File::create(path).unwrap();
        write!(
            file,
            "P6\n{} {}\n255\n",
            output.size.width, output.size.height
        )
        .unwrap();
        for pixel in report.frame.bytes.chunks_exact(4) {
            file.write_all(&[pixel[2], pixel[1], pixel[0]]).unwrap();
        }
    }
}

#[test]
fn a_full_reference_page_stays_in_the_text_cache_across_repaints() {
    let mut cache = CompositorTextRasterCache::default();
    let (projection, _, _) = reference_sheet_projection(
        &fixture::candidate(256),
        &fixture::catalog(256),
        1,
        Rect {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
        },
        |text, size| cache.measure(text, size),
    )
    .unwrap();
    let texts = projection
        .commands
        .into_iter()
        .filter_map(|c| {
            if let CompositorDisplayCommand::Text(t) = c {
                Some(HeadCompositorText {
                    node: t.node,
                    generation: t.generation,
                    geometry: t.geometry,
                    text: t.text,
                    font_size_millis: t.font_size_millis,
                    color: t.color,
                })
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    assert!(texts.len() > 128);
    for t in &texts {
        cache.raster_for(t).unwrap();
    }
    let initial = cache.stats();
    for t in &texts {
        cache.raster_for(t).unwrap();
    }
    assert_eq!(cache.stats().misses, initial.misses);
    assert_eq!(cache.stats().hits, texts.len());
    assert_eq!(cache.stats().evictions, 0);
}
