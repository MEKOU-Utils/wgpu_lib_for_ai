pub mod wgpu {
    pub mod wgpu_init;
}

pub use crate::wgpu::wgpu_init::WgpuState;
pub use pollster;
pub use bytemuck;