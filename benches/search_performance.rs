use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use std::fs::File;
use std::hint::black_box;
use std::io::{self, BufRead};
use std::time::Duration;

use ree_path_searcher::{PathSearcher, load_pak_files_to_memory};

fn load_pak_list(pak_list_file: &str) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let mut pak_file_list = vec![];
    let paks = File::open(pak_list_file)?;
    for line in io::BufReader::new(paks).lines() {
        let line = line?;
        if !line.is_empty() && !line.starts_with('#') {
            pak_file_list.push(line);
        }
    }
    Ok(pak_file_list)
}

fn bench_pak_search_memory(c: &mut Criterion) {
    // 使用测试用的pak文件列表
    let pak_list_file = "pak_list_short.txt";

    let pak_paths = match load_pak_list(pak_list_file) {
        Ok(paths) => paths,
        Err(e) => {
            eprintln!("无法加载PAK文件列表 {}: {}", pak_list_file, e);
            eprintln!("跳过基准测试。确保 {} 文件存在。", pak_list_file);
            return;
        }
    };

    if pak_paths.is_empty() {
        eprintln!("PAK文件列表为空，跳过基准测试");
        return;
    }

    // 加载PAK文件到内存
    let pak_data = match load_pak_files_to_memory(&pak_paths) {
        Ok(data) => data,
        Err(e) => {
            eprintln!("加载PAK文件到内存失败: {}", e);
            return;
        }
    };

    let total_size: usize = pak_data.iter().map(|d| d.len()).sum();
    println!(
        "已加载 {} 个PAK文件到内存，总大小: {:.2} MB",
        pak_data.len(),
        total_size as f64 / (1024.0 * 1024.0)
    );

    // 创建基于内存的搜索器
    let memory_searcher = match PathSearcher::from_memory(pak_data) {
        Ok(searcher) => searcher,
        Err(e) => {
            eprintln!("创建内存搜索器失败: {}", e);
            return;
        }
    };

    println!("内存搜索器文件数: {}", memory_searcher.pak_file_count());

    let mut group = c.benchmark_group("pak_search_comparison");

    rayon::ThreadPoolBuilder::new()
        .num_threads(4)
        .build_global()
        .unwrap();

    // 设置较长的测量时间，因为PAK文件搜索可能比较耗时
    group.measurement_time(Duration::from_secs(120));
    group.sample_size(10);

    // 基准测试：基于内存的PAK文件搜索
    group.bench_with_input(
        BenchmarkId::new("memory", "pak_files"),
        &memory_searcher,
        |b, searcher| {
            b.iter(|| {
                let result = searcher.search_pak_files().expect("搜索失败");
                black_box(result);
            });
        },
    );

    group.finish();
}

criterion_group!(benches, bench_pak_search_memory,);
criterion_main!(benches);
