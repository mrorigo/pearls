// Rust guideline compliant 2026-02-06

use criterion::{black_box, criterion_group, criterion_main, BatchSize, Criterion};
use pearls_core::{DepType, Dependency, IssueGraph, Pearl, Status, Storage};
use tempfile::TempDir;

fn build_pearls(count: usize) -> Vec<Pearl> {
    let mut pearls: Vec<Pearl> = Vec::with_capacity(count);
    for i in 0..count {
        let title = format!("Pearl {}", i);
        let mut pearl = Pearl::new(title, "bench".to_string());
        pearl.status = Status::Open;
        pearl.priority = (i % 5) as u8;
        if i > 0 {
            pearl.deps.push(Dependency {
                target_id: pearls[i - 1].id.clone(),
                dep_type: DepType::Blocks,
            });
        }
        pearls.push(pearl);
    }
    pearls
}

fn setup_storage(count: usize) -> (TempDir, Storage) {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let path = temp_dir.path().join("issues.jsonl");
    let mut storage = Storage::new(path).expect("Failed to create storage");
    let pearls = build_pearls(count);
    storage
        .save_all(&pearls)
        .expect("Failed to save benchmark pearls");
    (temp_dir, storage)
}

fn bench_load_all(c: &mut Criterion) {
    let (_temp_dir, storage) = setup_storage(1000);
    c.bench_function("load_all_1000", |b| {
        b.iter(|| black_box(storage.load_all()))
    });
}

fn bench_topological_sort(c: &mut Criterion) {
    let pearls = build_pearls(1000);
    let graph = IssueGraph::from_pearls(pearls).expect("Failed to build graph");
    c.bench_function("toposort_1000", |b| {
        b.iter(|| black_box(graph.topological_sort()))
    });
}

fn bench_create(c: &mut Criterion) {
    c.bench_function("create_pearl", |b| {
        b.iter_batched(
            || {
                let temp_dir = TempDir::new().expect("Failed to create temp dir");
                let path = temp_dir.path().join("issues.jsonl");
                let storage = Storage::new(path).expect("Failed to create storage");
                (temp_dir, storage)
            },
            |(_temp_dir, mut storage)| {
                let pearl = Pearl::new("Bench Pearl".to_string(), "bench".to_string());
                black_box(storage.save(&pearl)).expect("Failed to save pearl");
            },
            BatchSize::SmallInput,
        )
    });
}

fn bench_ready_queue(c: &mut Criterion) {
    let pearls = build_pearls(1000);
    let graph = IssueGraph::from_pearls(pearls).expect("Failed to build graph");
    c.bench_function("ready_queue_1000", |b| {
        b.iter(|| black_box(graph.ready_queue()))
    });
}

criterion_group!(
    benches,
    bench_load_all,
    bench_topological_sort,
    bench_create,
    bench_ready_queue
);
criterion_main!(benches);
