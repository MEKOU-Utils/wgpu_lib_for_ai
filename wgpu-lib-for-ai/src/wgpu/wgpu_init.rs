use std::collections::HashMap;

pub enum BufferType {
    ReadOnly,
    ReadWrite,
}

pub struct WgpuBuffer {
    pub buffer: wgpu::Buffer,
    pub size: usize,
} 

pub struct WgpuState {
    pub device:           wgpu::Device,
    pub queue:            wgpu::Queue,
    // 互換性維持のためのデフォルトパイプライン（"main" 用）
    pub compute_pipeline: wgpu::ComputePipeline,
    // エントリーポイント名ごとに事前ビルドしたパイプラインのキャッシュ
    pub pipelines:        HashMap<String, wgpu::ComputePipeline>,
    pub wgpu_bindgroup:   wgpu::BindGroup,
    pub buffers:          Vec<WgpuBuffer>,
}

impl WgpuState {
    
    pub async fn new_from_shader(
        shader_src: &str,
        buffer_config: &[(usize, BufferType)],
        entry_point: &[&str],
    ) -> anyhow::Result<Self> {
        let instance = wgpu::Instance::default();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference:       wgpu::PowerPreference::HighPerformance,
                compatible_surface:     None,
                force_fallback_adapter: false,
            })
            .await
            .ok_or_else(|| anyhow::anyhow!("GPU adapter が見つかりません"))?;

        let adapter_limits = adapter.limits();
            
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label:             Some("Device"),
                    required_features: wgpu::Features::empty(),
                    required_limits:   adapter_limits,
                    memory_hints:      wgpu::MemoryHints::default(),
                },
                None,
            )
            .await?;

        // 1. 各バッファのバインドレイアウトを動的に設定
        let mut layout_entries = Vec::new();
        for (i, (_, b_type)) in buffer_config.iter().enumerate() {
            layout_entries.push(wgpu::BindGroupLayoutEntry {
                binding: i as u32,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer { 
                    ty: wgpu::BufferBindingType::Storage { 
                        read_only: match b_type {
                            BufferType::ReadOnly => true,
                            BufferType::ReadWrite => false,
                        },
                    },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            });
        }

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("dynamic_bgl"),
            entries: &layout_entries,
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label:  Some("shader"),
            source: wgpu::ShaderSource::Wgsl(shader_src.into()),
        });

        // デフォルトの "main" パイプライン
        let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label:               Some("pipeline_main"),
            layout:              Some(&pipeline_layout),
            module:              &shader,
            entry_point:         Some("main"),
            compilation_options: Default::default(),
            cache:               None,
        });

        let mut pipelines = HashMap::new();
        
        // 互換性のためにデフォルトの "main" もキャッシュに入れておく
        pipelines.insert("main".to_string(), compute_pipeline.clone());

        // ★ 引数で指定されたエントリーポイント群をループで回してビルドする
        for &entry in entry_point {
            // "main" は上で既に入れているので重複ビルドをスキップ
            if entry == "main" { continue; }

            let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label:               Some(&format!("pipeline_{}", entry)),
                layout:              Some(&pipeline_layout),
                module:              &shader,
                entry_point:         Some(entry),
                compilation_options: Default::default(),
                cache:               None,
            });
            pipelines.insert(entry.to_string(), pipeline);
        }

        // 2. 実際のバッファ配列の生成
        let mut buffers = Vec::new();
        for (i, (size, b_type)) in buffer_config.iter().enumerate() {
            let usage = match b_type {
                BufferType::ReadOnly => wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                BufferType::ReadWrite => {
                    wgpu::BufferUsages::STORAGE 
                    | wgpu::BufferUsages::COPY_SRC 
                    | wgpu::BufferUsages::COPY_DST
                }
            };

            let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label:              Some(&format!("buffer_{}", i)),
                size:               (size * std::mem::size_of::<f32>()) as u64,
                usage,
                mapped_at_creation: false,
            });

            buffers.push(WgpuBuffer { buffer, size: *size });
        }

        // 3. バッファをBindGroupに結びつける
        let mut bg_entries = Vec::new();
        for (i, wgpu_buf) in buffers.iter().enumerate() {
            bg_entries.push(wgpu::BindGroupEntry {
                binding:  i as u32,
                resource: wgpu_buf.buffer.as_entire_binding(),
            });
        }

        let wgpu_bindgroup = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label:   Some("dynamic_bg"),
            layout:  &bind_group_layout,
            entries: &bg_entries,
        });

        Ok(Self {
            device,
            queue,
            compute_pipeline,
            pipelines,
            wgpu_bindgroup,
            buffers,
        })
    }

    /// 指定したインデックスのバッファへデータをアップロード
    pub fn upload(&self, idx: usize, data: &[f32]) {
        self.queue.write_buffer(
            &self.buffers[idx].buffer,
            0,
            bytemuck::cast_slice(data),
        );
    }

    /// コンピュートシェーダーの実行（デフォルト用）
    pub fn run_compute(&self, workgroups: u32) {
        let mut enc = self.device.create_command_encoder(
            &wgpu::CommandEncoderDescriptor { label: Some("compute") },
        );
        {
            let mut pass = enc.begin_compute_pass(
                &wgpu::ComputePassDescriptor { label: None, timestamp_writes: None },
            );
            pass.set_pipeline(&self.compute_pipeline);
            pass.set_bind_group(0, &self.wgpu_bindgroup, &[]);
            pass.dispatch_workgroups(workgroups, 1, 1);
        }
        self.queue.submit(Some(enc.finish()));
    }

    /// 事前ビルドされた特定のエントリーポイントを指定して高速実行
    pub fn run_compute_with_entry(&self, entry_point: &str, x: u32, y: u32, z: u32) {
        let pipeline = self.pipelines.get(entry_point)
            .expect(&format!("[WgpuState] 未登録のエントリーポイントです: {}", entry_point));

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: None,
                timestamp_writes: None,
            });
            compute_pass.set_pipeline(pipeline);
            compute_pass.set_bind_group(0, &self.wgpu_bindgroup, &[]);
            compute_pass.dispatch_workgroups(x, y, z);
        }
        self.queue.submit(Some(encoder.finish()));
        self.device.poll(wgpu::Maintain::Wait);
    }

    /// 指定したインデックスのバッファからデータを回収
    pub async fn get_data(&self, idx: usize) -> Vec<f32> {
        let target_buf = &self.buffers[idx];
        let size = (target_buf.size * std::mem::size_of::<f32>()) as u64;

        let staging = self.device.create_buffer(&wgpu::BufferDescriptor {
            label:              Some("staging"),
            size,
            usage:              wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let mut enc = self.device.create_command_encoder(
            &wgpu::CommandEncoderDescriptor { label: Some("read") },
        );
        enc.copy_buffer_to_buffer(&target_buf.buffer, 0, &staging, 0, size);
        self.queue.submit(Some(enc.finish()));

        let slice = staging.slice(..);
        let (tx, rx) = std::sync::mpsc::channel::<Result<(), wgpu::BufferAsyncError>>();
        slice.map_async(wgpu::MapMode::Read, move |r| { let _ = tx.send(r); });
        self.device.poll(wgpu::Maintain::Wait);
        rx.recv().unwrap().unwrap();

        let data   = slice.get_mapped_range();
        let result = bytemuck::cast_slice::<u8, f32>(&data).to_vec();
        drop(data);
        staging.unmap();
        result
    }
}