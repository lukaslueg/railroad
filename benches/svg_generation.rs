mod fixtures;

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};

fn construction_benches(c: &mut Criterion) {
    let mut group = c.benchmark_group("construction");
    group.bench_function("simple-sequence", |b| {
        b.iter(|| black_box(fixtures::simple_sequence_diagram()))
    });
    group.bench_function("vertical-grid", |b| {
        b.iter(|| black_box(fixtures::vertical_grid_diagram()))
    });
    group.bench_function("column-constraint", |b| {
        b.iter(|| black_box(fixtures::column_constraint_diagram()))
    });
    group.bench_function("create-table-stmt", |b| {
        b.iter(|| black_box(fixtures::create_table_stmt_diagram()))
    });
    group.finish();
}

fn serialization_benches(c: &mut Criterion) {
    let mut group = c.benchmark_group("svg-serialization");

    let simple_sequence = fixtures::simple_sequence_diagram();
    group.bench_function("simple-sequence", |b| {
        b.iter(|| black_box(simple_sequence.to_string()))
    });

    let vertical_grid = fixtures::vertical_grid_diagram();
    group.bench_function("vertical-grid", |b| {
        b.iter(|| black_box(vertical_grid.to_string()))
    });

    let column_constraint = fixtures::column_constraint_diagram();
    group.bench_function("column-constraint", |b| {
        b.iter(|| black_box(column_constraint.to_string()))
    });

    let create_table_stmt = fixtures::create_table_stmt_diagram();
    group.bench_function("create-table-stmt", |b| {
        b.iter(|| black_box(create_table_stmt.to_string()))
    });

    group.finish();
}

criterion_group!(benches, construction_benches, serialization_benches);
criterion_main!(benches);
