
use winit;
use winit::{
    EventsLoop,
    Window,
};

use vulkano_win;
use vulkano_win::VkSurfaceBuild;

use vulkano;
use vulkano::{
    buffer::{
        BufferAccess,
        BufferUsage,
        CpuAccessibleBuffer,
        cpu_pool::{
            CpuBufferPool,
        },
    },
    command_buffer::{
        AutoCommandBufferBuilder,
        DynamicState,
        pool::standard::StandardCommandPoolBuilder,
    },
    device::{
        Device,
        Queue,
    },
    framebuffer::{
        RenderPassAbstract,
        Framebuffer,
        FramebufferAbstract,
        Subpass,
    },
    image::{
        SwapchainImage,
    },
    instance::{
        Instance,
    },
    pipeline::{
        GraphicsPipeline,
        GraphicsPipelineAbstract,
        viewport::{
            Viewport,
        },
    },
    swapchain,
    swapchain::{
        AcquireError,
        PresentMode,
        Surface,
        SurfaceTransform,
        Swapchain,
        SwapchainCreationError,
    },
    sync::{
        now,
        GpuFuture,
    }
};

use std::sync::Arc;
use std::mem;

use cgmath;
use cgmath::{
    SquareMatrix,
};

use std;

use geometry::Vertex;
use geometry::Mesh;

mod vs {
    #[derive(VulkanoShader)]
    #[ty = "vertex"]
    #[src = "
#version 450
layout(location = 0) in vec3 position;
layout(set = 0, binding = 0) uniform Data {
    mat4 world;
    mat4 view;
    mat4 proj;
} uniforms;
void main() {
    mat4 worldview = uniforms.view * uniforms.world;
    gl_Position = uniforms.proj * worldview * vec4(position, 1.0);
}
"]
    struct Dummy;
}

mod fs {
    #[derive(VulkanoShader)]
    #[ty = "fragment"]
    #[src = "
#version 450
layout(location = 0) out vec4 f_color;
void main() {
    f_color = vec4(1.0, 0.0, 0.0, 1.0);
}
"]
    struct Dummy;
}

pub struct SimpleRenderer {
    instance: Arc<Instance>,
    physical_device_index: usize,
    device: Arc<Device>,
    queue: Arc<Queue>,
    swapchain: Arc<Swapchain<Window>>,
    images: Vec<Arc<SwapchainImage<Window>>>,
    framebuffers: Option<Vec<Arc<FramebufferAbstract + Send + Sync>>>,
    render_pass: Arc<RenderPassAbstract + Send + Sync>,
    uniform_buffer: CpuBufferPool<vs::ty::Data>,
    pipeline: Arc<GraphicsPipelineAbstract + Send + Sync>,
    events_loop: EventsLoop,
    window: Arc<Surface<Window>>,
    dimensions: [u32; 2],

    // definitely not good
    command_buffer_builder: AutoCommandBufferBuilder<StandardCommandPoolBuilder>,

    recreate_swapchain: bool,
    previous_frame_end: Box<GpuFuture>,
}

impl SimpleRenderer {
    pub fn create() -> Self {
        let instance = {
            let extensions = vulkano_win::required_extensions();
            Instance::new(None, &extensions, None).expect("failed to create Vulkan instance")
        };

        let physical = vulkano::instance::PhysicalDevice::enumerate(&instance)
            .next().expect("no device available");
        info!("Using physical device: {} (type: {:?})", physical.name(), physical.ty());

        let events_loop = winit::EventsLoop::new();
        let window = winit::WindowBuilder::new().build_vk_surface(&events_loop, instance.clone()).unwrap();

        let mut dimensions;

        let queue = physical.queue_families().find(|&q| {
            // We take the first queue that supports drawing to our window.
            q.supports_graphics() && window.is_supported(q).unwrap_or(false)
        }).expect("couldn't find a graphical queue family");

        let (device, mut queues) = {
            let device_ext = vulkano::device::DeviceExtensions {
                khr_swapchain: true,
                .. vulkano::device::DeviceExtensions::none()
            };

            Device::new(
                physical,
                physical.supported_features(),
                &device_ext,
                [(queue, 0.5)].iter().cloned()
            ).expect("failed to create device")
        };

        let queue = queues.next().unwrap();

        let (swapchain, images) = {
            let caps = window.capabilities(physical)
                .expect("failed to get surface capabilities");
            dimensions = caps.current_extent.unwrap_or([1024, 768]);
            let alpha = caps.supported_composite_alpha.iter().next().unwrap();
            let format = caps.supported_formats[0].0;
            Swapchain::new(
                device.clone(),
                window.clone(),
                caps.min_image_count,
                format,
                dimensions,
                1,
                caps.supported_usage_flags,
                &queue,
                SurfaceTransform::Identity,
                alpha,
                PresentMode::Fifo,
                true,
                None,
            ).expect("failed to create swapchain")
        };

        let uniform_buffer = vulkano::buffer::cpu_pool::CpuBufferPool::<vs::ty::Data>
        ::new(device.clone(), vulkano::buffer::BufferUsage::all());

        let render_pass = Arc::new(single_pass_renderpass!(device.clone(),
            attachments: {
                color: {
                    load: Clear,
                    store: Store,
                    format: swapchain.format(),
                    // TODO:
                    samples: 1,
                }
            },
            pass: {
                color: [color],
                depth_stencil: {}
            }
        ).unwrap());

        let vs = vs::Shader::load(device.clone()).expect("failed to create shader module");
        let fs = fs::Shader::load(device.clone()).expect("failed to create shader module");

        let pipeline = Arc::new(GraphicsPipeline::start()
            .vertex_input_single_buffer::<Vertex>()
            .vertex_shader(vs.main_entry_point(), ())
            .triangle_list()
            .viewports_dynamic_scissors_irrelevant(1)
            .fragment_shader(fs.main_entry_point(), ())
            .render_pass(Subpass::from(render_pass.clone(), 0).unwrap())
            .build(device.clone())
            .unwrap());

        let framebuffers: Option<Vec<Arc<FramebufferAbstract + Send + Sync>>> = None;

        // Initialization is finally finished!

        let recreate_swapchain = false;

        let previous_frame_end = Box::new(now(device.clone())) as Box<GpuFuture>;

        SimpleRenderer {
            instance: instance.clone(),
            physical_device_index: physical.index(),
            device,
            queue,
            swapchain,
            images,
            framebuffers,
            render_pass,
            uniform_buffer,
            pipeline,
            events_loop,
            window,
            dimensions,
            recreate_swapchain,
            previous_frame_end,
        }
    }

    pub fn mesh_from_vertices(&self, vertices: Vec<Vertex>) -> Mesh {
        Mesh::new(CpuAccessibleBuffer::from_iter(
            self.device.clone(),
            BufferUsage::all(),
            vertices.iter().cloned(),
        ).expect("failed to create buffer"))
    }

    pub fn start_frame(&mut self) {
        let proj = cgmath::perspective(
            cgmath::Rad(std::f32::consts::FRAC_PI_2),
            { self.dimensions[0] as f32 / self.dimensions[1] as f32 },
            0.01,
            100.0,
        );
        let view = cgmath::Matrix4::look_at(
            cgmath::Point3::new(0.0, 0.0, 0.0),
            cgmath::Point3::new(0.0, 0.0, -3.0),
            cgmath::Vector3::new(0.0, 1.0, 0.0),
        );
        let world = cgmath::Matrix4::identity();

        let vertex_buffer: Arc<BufferAccess + Send + Sync> = {
            CpuAccessibleBuffer::from_iter(self.device.clone(), BufferUsage::all(), [
                Vertex { position: [-0.5, -0.25, -2.0] },
                Vertex { position: [0.0, 0.5, -2.0] },
                Vertex { position: [0.25, -0.1, -2.0] }
            ].iter().cloned()).expect("failed to create buffer")
        };

        self.previous_frame_end.cleanup_finished();
        if self.recreate_swapchain {
            info!("Recreating swapchain.");
            let physical = vulkano::instance::PhysicalDevice::enumerate(&self.instance)
                .nth(self.physical_device_index)
                .expect("Couldn't get physical device.");
            self.dimensions = self.window.capabilities(physical)
                .expect("failed to get surface capabilities")
                .current_extent.unwrap_or([1024, 768]);

            let (new_swapchain, new_images) = match self.swapchain.recreate_with_dimension(self.dimensions) {
                Ok(r) => {
                    info!("Successfully recreated swapchain.");
                    r
                },
                Err(SwapchainCreationError::UnsupportedDimensions) => {
                    info!("Failed to recreate swapchain; unsupported dimensions.");
                    return;
                },
                Err(err) => panic!("{:?}", err)
            };

            mem::replace(&mut self.swapchain, new_swapchain);
            mem::replace(&mut self.images, new_images);

            self.framebuffers = None;
            self.recreate_swapchain = false;
        }

        // Because framebuffers contains an Arc on the old swapchain, we need to
        // recreate framebuffers as well.
        if self.framebuffers.is_none() {
            let new_framebuffers: Option<Vec<Arc<FramebufferAbstract + Send + Sync>>> = Some(self.images.iter().map(|image| {
                Arc::new(Framebuffer::start(self.render_pass.clone())
                    .add(image.clone()).unwrap()
                    .build().unwrap()) as Arc<FramebufferAbstract + Send + Sync>
            }).collect::<Vec<Arc<FramebufferAbstract + Send + Sync>>>());
            mem::replace(&mut self.framebuffers, new_framebuffers);
        }

        let uniform_buffer_subbuffer = {
            let uniform_data = vs::ty::Data {
                world: world.into(),
                view: view.into(),
                proj: proj.into(),
            };

            self.uniform_buffer.next(uniform_data).unwrap()
        };

        let set = Arc::new(
            vulkano::descriptor::descriptor_set::PersistentDescriptorSet::start(self.pipeline.clone(), 0)
                .add_buffer(uniform_buffer_subbuffer)
                .unwrap()
                .build()
                .unwrap()
        );

        let (image_num, acquire_future) =
            match swapchain::acquire_next_image(self.swapchain.clone(), None) {
                Ok(r) => r,
                Err(AcquireError::OutOfDate) => {
                    self.recreate_swapchain = true;
                    return;
                },
                Err(err) => panic!("{:?}", err)
            };

        self.command_buffer = AutoCommandBufferBuilder::primary_one_time_submit(
            self.device.clone(),
            self.queue.family()
        ).unwrap()
            .begin_render_pass(
                self.framebuffers.as_ref().unwrap()[image_num].clone(),
                false,
                vec![[0.0, 0.0, 1.0, 1.0].into()]
            )
            .unwrap();
    }

    pub fn draw(&mut self, mesh: &Mesh) {

    }

    pub fn do_stuff(&mut self) {

            .draw(
                self.pipeline.clone(),
                &DynamicState {
                    line_width: None,
                    // TODO: Find a way to do this without having to dynamically allocate a Vec every frame.
                    viewports: Some(vec![Viewport {
                        origin: [0.0, 0.0],
                        dimensions: [self.dimensions[0] as f32, self.dimensions[1] as f32],
                        depth_range: 0.0 .. 1.0,
                    }]),
                    scissors: None,
                },
                // uhh
                vec![vertex_buffer.clone()],
                set.clone(),
                ()
            )
            .unwrap()
            .end_render_pass()
            .unwrap()
            .build()
            .unwrap();

        let previous_frame_end = mem::replace(
            &mut self.previous_frame_end,
            Box::new(now(self.device.clone())) as Box<GpuFuture>,
        );
        let future = previous_frame_end
            .join(acquire_future)
            .then_execute(self.queue.clone(), command_buffer).unwrap()
            .then_swapchain_present(self.queue.clone(), self.swapchain.clone(), image_num)
            .then_signal_fence_and_flush().unwrap();
        self.previous_frame_end = Box::new(future) as Box<_>;

        let mut done = false;
        let mut recreate_swapchain = self.recreate_swapchain;
        self.events_loop.poll_events(|ev| {
            match ev {
                winit::Event::WindowEvent { event: winit::WindowEvent::CloseRequested, .. } => {
                    done = true;
                },
                winit::Event::WindowEvent { event: winit::WindowEvent::Resized(_), .. } => recreate_swapchain = true,
                _ => ()
            }
        });
        self.recreate_swapchain = recreate_swapchain;
        if done { return; }
    }
}

