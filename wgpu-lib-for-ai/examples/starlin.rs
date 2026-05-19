use wgpu_lib_for_ai::{WgpuState, wgpu::wgpu_init::BufferType};

fn main() {

    let config = vec![
        (100, BufferType::ReadWrite),
        (100, BufferType::ReadWrite),
    ];

    let entry = vec![
        "sub_main",
    ];

    // 1. 初期化（Result なので unwrap）
    let wgpu_state = pollster::block_on(WgpuState::new_from_shader(
        include_str!("shader.wgsl"),
        &config,
        &entry,
    )).unwrap();

    // 2. データをGPUにアップロード
    let input: Vec<f32> = (0..100).map(|i| i as f32).collect();
    wgpu_state.upload(0, &input);

    // 3. コンピュートシェーダーを実行
    wgpu_state.run_compute(16); // 1024要素 / workgroup_size(64) = 16

    // 4. 結果を取得（async なので block_on）
    let data = pollster::block_on(wgpu_state.get_data(0));
    println!("CPU output {:?}", &input[..10]);
    println!("GPU output (first 10): {:?}", &data[..10]);

    wgpu_state.run_compute_with_entry("sub_main", 16, 1, 1);
    let data = pollster::block_on(wgpu_state.get_data(0));
    println!("GPU output {:?}", &data[..10]);
}