//! Benchmarks für den Renderer-Kern.
//!
//! Gemessen wird, was heute existiert: Kerzenerzeugung, Viewport-Abbildung,
//! Sichtbarkeitsfilter und der Zeichenbefehls-Strom der Werkzeuge. Der eigentliche
//! Renderloop steckt noch in der WASM-Fassade (siehe plan/03-meilensteine.md, M2) —
//! sobald er im Kern liegt, gehört er hierher.
//!
//! Der frühere Bench aus `loomchart` maß `ChartRenderer`, also toten Code, der im
//! ausgelieferten Pfad nie lief.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use kestrel_loom::core::{
    Candle, CandleGenerator, GeneratorConfig, PriceRange, TimeRange, Viewport,
};
use kestrel_loom::rendering::cull_candles;
use kestrel_loom::tools::{ChartTool, ToolManager, ToolNode, TrendLine};
use kestrel_loom::BatchRenderer;

fn make_candles(n: usize) -> Vec<Candle> {
    let mut generator = CandleGenerator::new(GeneratorConfig::crypto().with_seed(42));
    generator.generate(n)
}

fn fitted_viewport(candles: &[Candle]) -> Viewport {
    let mut viewport = Viewport::new(1280, 720);
    let min = candles.iter().map(|c| c.l).fold(f64::MAX, f64::min);
    let max = candles.iter().map(|c| c.h).fold(f64::MIN, f64::max);
    viewport.fit_to_data(
        TimeRange {
            start: candles[0].time,
            end: candles[candles.len() - 1].time,
        },
        PriceRange { min, max },
    );
    viewport
}

fn bench_generate(c: &mut Criterion) {
    let mut group = c.benchmark_group("generate_candles");
    for size in [1_000usize, 10_000, 100_000] {
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &n| {
            b.iter(|| black_box(make_candles(n)));
        });
    }
    group.finish();
}

fn bench_project(c: &mut Criterion) {
    let mut group = c.benchmark_group("project_to_pixels");
    for size in [1_000usize, 10_000, 100_000] {
        let candles = make_candles(size);
        let viewport = fitted_viewport(&candles);
        group.bench_with_input(BenchmarkId::from_parameter(size), &candles, |b, candles| {
            b.iter(|| {
                let mut acc = 0.0;
                for candle in candles {
                    acc += viewport.time_to_x(candle.time) + viewport.price_to_y(candle.c);
                }
                black_box(acc)
            });
        });
    }
    group.finish();
}

fn bench_cull(c: &mut Criterion) {
    let mut group = c.benchmark_group("cull_candles");
    for size in [10_000usize, 100_000] {
        let candles = make_candles(size);
        let mut viewport = fitted_viewport(&candles);
        viewport.zoom(0.1, None); // nur ein Ausschnitt sichtbar
        group.bench_with_input(BenchmarkId::from_parameter(size), &candles, |b, candles| {
            b.iter(|| black_box(cull_candles(candles, &viewport).len()));
        });
    }
    group.finish();
}

fn bench_tool_commands(c: &mut Criterion) {
    let candles = make_candles(1_000);
    let viewport = fitted_viewport(&candles);

    let mut tools = ToolManager::new();
    for i in 0..20 {
        let mut line = TrendLine::new(tools.generate_id("trend"));
        let base = candles[0].time;
        line.nodes_mut()
            .push(ToolNode::new(base + i * 60, candles[0].c));
        line.nodes_mut()
            .push(ToolNode::new(base + (i + 10) * 60, candles[0].c * 1.02));
        tools.add_tool(Box::new(line));
    }

    c.bench_function("tools_to_commands_20", |b| {
        b.iter(|| {
            let mut recorder = BatchRenderer::new(1280, 720);
            tools.render_all(&mut recorder, &viewport);
            black_box(recorder.commands().len())
        });
    });
}

criterion_group!(
    benches,
    bench_generate,
    bench_project,
    bench_cull,
    bench_tool_commands
);
criterion_main!(benches);
