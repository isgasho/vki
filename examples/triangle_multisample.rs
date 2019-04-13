use vki::{
    BindGroupBinding, BindGroupDescriptor, BindGroupLayoutBinding, BindGroupLayoutDescriptor, BindingResource,
    BindingType, BlendDescriptor, BlendFactor, BlendOperation, BufferDescriptor, BufferUsageFlags, Color,
    ColorStateDescriptor, ColorWriteFlags, CullMode, DeviceDescriptor, Extent3D, FrontFace, IndexFormat,
    InputStateDescriptor, InputStepMode, Instance, LoadOp, PipelineLayoutDescriptor, PipelineStageDescriptor,
    PrimitiveTopology, RasterizationStateDescriptor, RenderPassColorAttachmentDescriptor, RenderPassDescriptor,
    RenderPipelineDescriptor, RequestAdapterOptions, ShaderModuleDescriptor, ShaderStageFlags, StoreOp,
    SwapchainDescriptor, TextureDescriptor, TextureDimension, TextureFormat, TextureUsageFlags,
    VertexAttributeDescriptor, VertexFormat, VertexInputDescriptor,
};

use winit::dpi::LogicalSize;
use winit::event::{Event, KeyboardInput, StartCause, VirtualKeyCode, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::platform::desktop::EventLoopExtDesktop;

use std::borrow::Cow;
use std::time::{Duration, Instant};

macro_rules! offset_of {
    ($base:path, $field:ident) => {{
        #[allow(unused_unsafe)]
        unsafe {
            let b: $base = std::mem::uninitialized();
            let offset = (&b.$field as *const _ as isize) - (&b as *const _ as isize);
            std::mem::forget(b);
            offset as _
        }
    }};
}

fn main() -> Result<(), Box<std::error::Error>> {
    if std::env::var("VK_INSTANCE_LAYERS").is_err() {
        std::env::set_var("VK_INSTANCE_LAYERS", "VK_LAYER_LUNARG_standard_validation");
    }

    let _ = pretty_env_logger::try_init();

    let mut event_loop = EventLoop::new();

    let (mut window_width, mut window_height) = (1024, 768);

    let window = winit::window::WindowBuilder::new()
        .with_title("triangle.rs")
        .with_dimensions(LogicalSize::new(window_width as _, window_height as _))
        .with_visibility(false)
        .build(&event_loop)?;

    let instance = Instance::new()?;
    let adapter_options = RequestAdapterOptions::default();
    let adapter = instance.request_adaptor(adapter_options)?;
    println!("Adapter: {}", adapter.name());

    let surface_descriptor = vki::winit_surface_descriptor!(&window);

    let surface = instance.create_surface(&surface_descriptor)?;

    let device_desc = DeviceDescriptor::default().with_surface_support(&surface);
    let device = adapter.create_device(device_desc)?;

    let formats = device.get_supported_swapchain_formats(&surface)?;
    println!("Supported swapchain formats: {:?}", formats);

    let swapchain_format = TextureFormat::B8G8R8A8Unorm;
    assert!(formats.contains(&swapchain_format));

    let swapchain_desc = SwapchainDescriptor {
        surface: &surface,
        format: swapchain_format,
        usage: TextureUsageFlags::OUTPUT_ATTACHMENT,
    };

    let mut swapchain = device.create_swapchain(swapchain_desc, None)?;
    let mut last_frame_time = Instant::now();
    window.show();

    let vertex_shader = device.create_shader_module(ShaderModuleDescriptor {
        code: Cow::Borrowed(include_bytes!("shaders/triangle.vert.spv")),
    })?;

    let fragment_shader = device.create_shader_module(ShaderModuleDescriptor {
        code: Cow::Borrowed(include_bytes!("shaders/triangle.frag.spv")),
    })?;

    let bind_group_layout = device.create_bind_group_layout(BindGroupLayoutDescriptor {
        bindings: &[BindGroupLayoutBinding {
            binding: 0,
            visibility: ShaderStageFlags::VERTEX,
            binding_type: BindingType::UniformBuffer,
        }],
    })?;

    let pipeline_layout = device.create_pipeline_layout(PipelineLayoutDescriptor {
        bind_group_layouts: vec![bind_group_layout.clone()],
    })?;

    #[repr(C)]
    #[derive(Copy, Clone, Debug)]
    struct Uniforms {
        clip: [[f32; 4]; 4],
        time: f32,
    }

    let uniforms_size_bytes = std::mem::size_of::<Uniforms>();

    let uniform_buffer = device.create_buffer(BufferDescriptor {
        size: uniforms_size_bytes,
        usage: BufferUsageFlags::UNIFORM | BufferUsageFlags::TRANSFER_DST,
    })?;

    #[rustfmt::skip]
    let mut uniforms = Uniforms {
        // Vulkan clip space has inverted Y and half Z
        // https://github.com/LunarG/VulkanSamples/commit/0dd36179880238014512c0637b0ba9f41febe803
        // https://matthewwellings.com/blog/the-new-vulkan-coordinate-system/
        // http://anki3d.org/vulkan-coordinate-system/
        clip: [
            [1.0,  0.0, 0.0, 0.0],
            [0.0, -1.0, 0.0, 0.0],
            [0.0,  0.0, 0.5, 0.0],
            [0.0,  0.0, 0.5, 1.0],
        ],
        time: 0.0,
    };

    let bind_group = device.create_bind_group(BindGroupDescriptor {
        layout: bind_group_layout.clone(),
        bindings: vec![BindGroupBinding {
            binding: 0,
            resource: BindingResource::Buffer(uniform_buffer.clone(), 0..uniforms_size_bytes),
        }],
    })?;

    #[repr(C)]
    #[derive(Copy, Clone, Debug)]
    struct Vertex {
        position: [f32; 3],
        color: [f32; 3],
    }

    #[rustfmt::skip]
    let vertices = &[
        Vertex { position: [-0.5, -0.5, 0.0], color: [1.0, 0.0, 0.0] },
        Vertex { position: [ 0.5, -0.5, 0.0], color: [0.0, 1.0, 0.0] },
        Vertex { position: [ 0.0,  0.5, 0.0], color: [0.0, 0.0, 1.0] },
    ];

    let vertices_size_bytes = std::mem::size_of::<Vertex>() * vertices.len();

    let vertex_buffer = device.create_buffer(BufferDescriptor {
        size: vertices_size_bytes,
        usage: BufferUsageFlags::VERTEX | BufferUsageFlags::TRANSFER_DST,
    })?;

    let staging_vertex_buffer = device.create_buffer_mapped(BufferDescriptor {
        size: vertices_size_bytes,
        usage: BufferUsageFlags::TRANSFER_SRC | BufferUsageFlags::MAP_WRITE,
    })?;

    staging_vertex_buffer.write(0, vertices)?;

    let mut encoder = device.create_command_encoder()?;

    encoder.copy_buffer_to_buffer(
        staging_vertex_buffer.unmap(),
        0,
        vertex_buffer.clone(),
        0,
        vertices_size_bytes,
    );

    device.get_queue().submit(&[encoder.finish()?])?;

    let mut output_texture_descriptor = TextureDescriptor {
        sample_count: 8,
        usage: TextureUsageFlags::OUTPUT_ATTACHMENT,
        format: swapchain_format,
        dimension: TextureDimension::D2,
        array_layer_count: 1,
        mip_level_count: 1,
        size: Extent3D {
            width: window_width,
            height: window_height,
            depth: 1,
        },
    };

    let mut output_texture = device.create_texture(output_texture_descriptor)?;
    let mut output_texture_view = output_texture.create_default_view()?;

    let color_replace = BlendDescriptor {
        src_factor: BlendFactor::One,
        dst_factor: BlendFactor::Zero,
        operation: BlendOperation::Add,
    };

    let render_pipeline_descriptor = RenderPipelineDescriptor {
        layout: pipeline_layout.clone(),
        primitive_topology: PrimitiveTopology::TriangleList,
        vertex_stage: PipelineStageDescriptor {
            entry_point: Cow::Borrowed("main"),
            module: vertex_shader,
        },
        fragment_stage: PipelineStageDescriptor {
            entry_point: Cow::Borrowed("main"),
            module: fragment_shader,
        },
        input_state: InputStateDescriptor {
            index_format: IndexFormat::U16,
            inputs: vec![VertexInputDescriptor {
                input_slot: 0,
                step_mode: InputStepMode::Vertex,
                stride: std::mem::size_of::<Vertex>(),
            }],
            attributes: vec![
                VertexAttributeDescriptor {
                    input_slot: 0,
                    format: VertexFormat::Float3,
                    offset: offset_of!(Vertex, position),
                    shader_location: 0,
                },
                VertexAttributeDescriptor {
                    input_slot: 0,
                    format: VertexFormat::Float3,
                    offset: offset_of!(Vertex, color),
                    shader_location: 1,
                },
            ],
        },
        color_states: vec![ColorStateDescriptor {
            format: swapchain_format,
            write_mask: ColorWriteFlags::ALL,
            color_blend: color_replace,
            alpha_blend: color_replace,
        }],
        depth_stencil_state: None,
        rasterization_state: RasterizationStateDescriptor {
            front_face: FrontFace::Ccw,
            cull_mode: CullMode::None,
            depth_bias: 0,
            depth_bias_slope_scale: 0.0,
            depth_bias_clamp: 0.0,
        },
        sample_count: output_texture_descriptor.sample_count,
    };

    let pipeline = device.create_render_pipeline(render_pipeline_descriptor)?;

    let start = Instant::now();

    let mut last_fps_time = Instant::now();
    let mut frame_count = 0;

    event_loop.run_return(|event, _target, control_flow| {
        let mut handle_event = || {
            match event {
                Event::NewEvents(StartCause::Init) | Event::NewEvents(StartCause::ResumeTimeReached { .. }) => {
                    window.request_redraw();
                }
                Event::WindowEvent {
                    event: WindowEvent::CloseRequested,
                    ..
                }
                | Event::WindowEvent {
                    event:
                        WindowEvent::KeyboardInput {
                            input:
                                KeyboardInput {
                                    virtual_keycode: Some(VirtualKeyCode::Escape),
                                    ..
                                },
                            ..
                        },
                    ..
                } => {
                    *control_flow = ControlFlow::Exit;
                }
                Event::WindowEvent {
                    event: WindowEvent::Resized(LogicalSize { width, height }),
                    ..
                } => {
                    let (width, height) = (width as u32, height as u32);
                    window_width = width as _;
                    window_height = height as _;
                    if width > 0 && height > 0 {
                        output_texture_descriptor.size.width = width;
                        output_texture_descriptor.size.height = height;
                        output_texture = device.create_texture(output_texture_descriptor)?;
                        output_texture_view = output_texture.create_default_view()?;
                        swapchain = device.create_swapchain(swapchain_desc, Some(&swapchain))?;
                    }
                }
                Event::WindowEvent {
                    event: WindowEvent::RedrawRequested,
                    ..
                } => {
                    let frame_time = Instant::now();

                    if window_width <= 0 || window_height <= 0 {
                        last_frame_time = frame_time;
                        *control_flow = ControlFlow::WaitUntil(last_frame_time + Duration::from_millis(16));
                        return Ok(());
                    }

                    frame_count += 1;

                    if last_fps_time.elapsed() > Duration::from_millis(1000) {
                        println!("FPS: {}", frame_count);
                        frame_count = 0;
                        last_fps_time = Instant::now();
                    }

                    let frame = swapchain.acquire_next_image()?;

                    uniforms.time = (start.elapsed().as_millis() as f32) / 1000.0;
                    uniform_buffer.set_sub_data(0, &[uniforms])?;

                    let mut encoder = device.create_command_encoder()?;
                    let mut render_pass = encoder.begin_render_pass(RenderPassDescriptor {
                        color_attachments: &[RenderPassColorAttachmentDescriptor {
                            attachment: &output_texture_view,
                            clear_color: Color {
                                r: 0.1,
                                g: 0.1,
                                b: 0.1,
                                a: 1.0,
                            },
                            load_op: LoadOp::Clear,
                            store_op: StoreOp::Store,
                            resolve_target: Some(&frame.view),
                        }],
                        depth_stencil_attachment: None,
                    });

                    render_pass.set_pipeline(&pipeline);
                    render_pass.set_vertex_buffers(0, &[vertex_buffer.clone()], &[0]);
                    render_pass.set_bind_group(0, &bind_group, None);
                    render_pass.draw(3, 1, 0, 1);
                    render_pass.end_pass();

                    let queue = device.get_queue();
                    queue.submit(&[encoder.finish()?])?;

                    let queue = device.get_queue();
                    queue.present(frame)?;

                    *control_flow = ControlFlow::WaitUntil(last_frame_time + Duration::from_millis(16));
                    last_frame_time = frame_time;
                }
                _ => {}
            }
            Ok(())
        };
        let result: Result<(), Box<std::error::Error>> = handle_event();
        result.expect("event loop error");
    });

    Ok(())
}