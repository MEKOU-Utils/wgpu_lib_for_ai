///Wgpu constructer
/// init -> upload -> run_compute -> get_data

pub struct WgpuState {
    pub device:           wgpu::Device,
    pub queue:            wgpu::Queue,
    pub compute_pipeline: wgpu::ComputePipeline,
    wgpu_bindgroup:       wgpu::BindGroup,
    pub storage_buffer:   wgpu::Buffer,
    buf_size:             usize,
}

impl WgpuState {
    pub fn update_shader(&mut self, new_shader_src: &str) {
        // Device を再利用してパイプラインと BindGroup だけ作り直す
        let (new_pipeline, new_bindgroup, _) = 
            Self::build(&self.device, new_shader_src, self.buf_size);
        
        self.compute_pipeline = new_pipeline;
        self.wgpu_bindgroup = new_bindgroup;
        // storage_buffer はそのまま使い回すので、データは維持される
    }

    ///new_from_shader(shader_src: &str, buf_size: usize) -> `anyhow::Result<Self>`
    /// shader_src: shader_name.wgsl source
    /// buf_size: buffersize
    /// # Example
    /// ```
    ///     let mut wgpu_state = pollster::block_on(WgpuState::new_from_shader(
    ///     include_str!("shader.wgsl"),
    ///     1024,
    ///     )).unwrap();
    /// ```
    // ---------------------------------------------------------------
    // constructer
    // ---------------------------------------------------------------
    pub async fn new_from_shader(shader_src: &str, buf_size: usize) -> anyhow::Result<Self> {
        // 1. インスタンス生成
        let instance = wgpu::Instance::default();

        // 2. アダプター（物理GPU）の取得
        //    request_adapter は Option<Adapter> を返す
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference:       wgpu::PowerPreference::HighPerformance,
                compatible_surface:     None,
                force_fallback_adapter: false,
            })
            .await?;

        // 3. デバイスとキューの取得
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label:                 Some("Device"),
                    required_features:     wgpu::Features::empty(),
                    required_limits:       wgpu::Limits::default(),
                    memory_hints:          wgpu::MemoryHints::default(),
                    trace:                 wgpu::Trace::Off,
                    experimental_features: wgpu::ExperimentalFeatures::disabled(),
                },
            )
            .await?;

        // 4. パイプライン・BindGroup・バッファを構築
        let (compute_pipeline, wgpu_bindgroup, storage_buffer) =
            Self::build(&device, shader_src, buf_size);

        Ok(Self {
            device,
            queue,
            compute_pipeline,
            wgpu_bindgroup,
            storage_buffer,
            buf_size,
        })
    }

    // ---------------------------------------------------------------
    // build helper
    // ---------------------------------------------------------------
    fn build(
        device:   &wgpu::Device,
        src:      &str,
        buf_size: usize,
    ) -> (wgpu::ComputePipeline, wgpu::BindGroup, wgpu::Buffer) {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label:  Some("shader"),
            source: wgpu::ShaderSource::Wgsl(src.into()),
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label:               Some("pipeline"),
            layout:              None,
            module:              &shader,
            entry_point:         Some("main"),
            compilation_options: Default::default(),
            cache:               None,
        });
        let storage_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label:              Some("storage"),
            size:               (buf_size * std::mem::size_of::<f32>()) as u64,
            usage:              wgpu::BufferUsages::STORAGE
                              | wgpu::BufferUsages::COPY_SRC
                              | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bgl        = pipeline.get_bind_group_layout(0);
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label:   Some("bg"),
            layout:  &bgl,
            entries: &[wgpu::BindGroupEntry {
                binding:  0,
                resource: storage_buffer.as_entire_binding(),
            }],
        });
        (pipeline, bind_group, storage_buffer)
    }

    ///upload(&self, data: &[f32])
    /// Example
    /// ```
    ///     let input: Vec<f32> = (0..1024).map(|i| i as f32).collect();
    ///     wgpu_state.upload(&input);
    /// ```
    // ---------------------------------------------------------------
    // Write data to GPU Buffer
    // ---------------------------------------------------------------
    pub fn upload(&self, data: &[f32]) {
        self.queue.write_buffer(
            &self.storage_buffer,
            0,
            bytemuck::cast_slice(data),
        );
    }

    ///run_compute(&self, workgroup: u32)
    /// workgroups: workgroup value
    /// ```
    ///     wgpu_state.run_compute(16);
    /// ```
    // ---------------------------------------------------------------
    // Execute compute shader
    // ---------------------------------------------------------------
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

    ///get_data(&self) -> `Vec<f32>`
    /// Example
    /// ```
    ///     let data = pollster::block_on(wgpu_state.get_data());
    /// ```
    // ---------------------------------------------------------------
    // Read from GPU buffer
    // ---------------------------------------------------------------
    pub async fn get_data(&self) -> Vec<f32> {
        let size = (self.buf_size * std::mem::size_of::<f32>()) as u64;

        let staging = self.device.create_buffer(&wgpu::BufferDescriptor {
            label:              Some("staging"),
            size,
            usage:              wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let mut enc = self.device.create_command_encoder(
            &wgpu::CommandEncoderDescriptor { label: Some("read") },
        );
        enc.copy_buffer_to_buffer(&self.storage_buffer, 0, &staging, 0, size);
        self.queue.submit(Some(enc.finish()));

        let slice        = staging.slice(..);
        let (tx, rx)     = std::sync::mpsc::channel::<Result<(), wgpu::BufferAsyncError>>();
        slice.map_async(wgpu::MapMode::Read, move |r| { let _ = tx.send(r); });
        self.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        rx.recv().unwrap().unwrap();

        let data   = slice.get_mapped_range();
        let result = bytemuck::cast_slice::<u8, f32>(&data).to_vec();
        drop(data);
        staging.unmap();
        result
    }
}