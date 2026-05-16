use wgpu_lib_for_ai::WgpuState;

fn main() {
    // 1. 初期化（Result なので unwrap）
    let mut wgpu_state = pollster::block_on(WgpuState::new_from_shader(
        include_str!("shader.wgsl"),
        1024,
    )).unwrap();

    // 2. データをGPUにアップロード
    let input: Vec<f32> = (0..1024).map(|i| i as f32).collect();
    wgpu_state.upload(&input);

    // 3. コンピュートシェーダーを実行
    wgpu_state.run_compute(16); // 1024要素 / workgroup_size(64) = 16

    // 4. 結果を取得（async なので block_on）
    let data = pollster::block_on(wgpu_state.get_data());
    println!("GPU output (first 10): {:?}", &data[..10]);

    // shaderを差し替え（wgpu_stateはmutが必要）
    wgpu_state.update_shader(include_str!("shader2.wgsl"));
    
    wgpu_state.upload(&input);
    wgpu_state.run_compute(16);

    let data = pollster::block_on(wgpu_state.get_data());
    println!("GPU output (first 10): {:?}", &data[..10]);

}