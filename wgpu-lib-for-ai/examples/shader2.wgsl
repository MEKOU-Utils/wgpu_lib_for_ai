// binding 0 のみ。ライブラリ側の BindGroup に合わせる
@group(0) @binding(0) var<storage, read_write> buf: array<f32>;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) global: vec3<u32>) {
    let index = global.x;
    if index >= arrayLength(&buf) { return; }
    // 入力を 2 倍にするサンプル
    buf[index] = buf[index] * 2.0;
}