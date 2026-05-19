# wgpu_lib_for_ai

## Lightweight wgpu wrapper

this is a wrapper what hide complicated wgpu initialization, and GPGPU Pipeline.

## Feature
- Headless by default:Works without a window system. Optimized for AI and backend computation.(windowなしでの動作がデフォルト、CPU側でWinとつなげればよい、AIなどに特化)
- Zero-boilerplate:Automates management of Device, Queue, and Pipeline.(Device,Queue,Pipelineの管理を自動化)
- Easy Data Transfer:Straightforward **upload** and **get_data** API for CPU/GPU communication.(upload, get_dataの簡単なパイプラインのみ)

## dependencies
```
[dependencies]
wgpu_lib_for_ai = "0.1.5"
pollster = "0.4"
wgpu = "24" //bevyとの整合性を保ちたい
```

## import
```
use wgpu_lib_for_ai::pollster; // ライブラリ経由で呼び出せる
use wgpu_lib_for_ai::bytemuck;
use wgpu_lib_for_ai::{WgpuState, wgpu::wgpu_init::BufferType};
```

//main
```
    let config = vec![
        (100, BufferType::ReadWrite),
        (100, BufferType::ReadWrite),
    ];

    let entry = vec![
        "sub_main",
    ];

    // init
    let wgpu_state = pollster::block_on(WgpuState::new_from_shader(
        include_str!("shader.wgsl"),
        &config,
        &entry,
    )).unwrap();

    // upload to GPU
    let input: Vec<f32> = (0..100).map(|i| i as f32).collect();
    wgpu_state.upload(0, &input);

    // run compute(main is default entry)
    wgpu_state.run_compute(16); // 1024要素 / workgroup_size(64) = 16

    // get data
    let data = pollster::block_on(wgpu_state.get_data(0));
    println!("CPU output {:?}", &input[..10]);
    println!("GPU output (first 10): {:?}", &data[..10]);

    wgpu_state.run_compute_with_entry("sub_main", 16, 1, 1);
    let data = pollster::block_on(wgpu_state.get_data(0));
    println!("GPU output {:?}", &data[..10]);
```

wgsl code for test
```
@group(0) @binding(0) var<storage, read_write> buf: array<f32>;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) global: vec3<u32>) {
    let index = global.x;
    if index >= arrayLength(&buf) { return; }
    // 入力を 2 倍にするサンプル
    buf[index] = buf[index] * 2.0;
}

@compute @workgroup_size(64)
fn sub_main(@builtin(global_invocation_id) global: vec3<u32>) {
    let index = global.x;
    if index >= arrayLength(&buf) { return; }
    buf[index] = buf[index] * 2.0;
}
```