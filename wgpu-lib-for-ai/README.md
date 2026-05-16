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
wgpu_lib_for_ai = "0.1.0"
pollster = "0.4"
```

## import
```
use wgpu_lib_for_ai::{WgpuState, pollster, bytemuck};
```

//main
```
    // 1. initalize (using pollster to block on async)
    let mut wgpu_state = pollster::block_on(WgpuState::new_from_shader(
        include_str!("shader.wgsl"),
        1024,
    )).unwrap();

    // 2. upload to gpu
    let input: Vec<f32> = (0..1024).map(|i| i as f32).collect();
    wgpu_state.upload(&input);

    // 3. execute compute shader
    wgpu_state.run_compute(16); // if workgroup_size(64) = 1024 elements

    // 4. 結果を取得（async なので block_on）
    let data = pollster::block_on(wgpu_state.get_data());
    println!("GPU output (first 10): {:?}", &data[..10]);
```

wgsl code for test
```
// binding 0 のみ。ライブラリ側の BindGroup に合わせる
@group(0) @binding(0) var<storage, read_write> buf: array<f32>;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) global: vec3<u32>) {
    let index = global.x;
    if index >= arrayLength(&buf) { return; }
    // 入力を 2 倍にするサンプル
    buf[index] = buf[index] * 2.0;
}
```