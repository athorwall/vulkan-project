
use std::sync::Arc;
use vulkano::device::Device;
use vulkano::device::Queue;
use winit::Window;
use vulkano::swapchain::Surface;
use winit::EventsLoop;
use vulkano_win;
use vulkano;
use winit;
use vulkano_win::VkSurfaceBuild;
use vulkano::swapchain::Swapchain;
use vulkano::image::SwapchainImage;
use vulkano::instance::PhysicalDevice;
use geometry::Vertex;
use obj::ObjModel;
use vulkano::image::ImageViewAccess;
use vulkano::sync::GpuFuture;
use vulkano::sampler::Sampler;
use image;
use vulkano::framebuffer::RenderPassAbstract;
use vulkano::framebuffer::FramebufferAbstract;
use vulkano::pipeline::GraphicsPipelineAbstract;
use vulkano::pipeline::shader::GraphicsEntryPointAbstract;
use vulkano::image::AttachmentImage;
use vulkano::format::D16Unorm;
use vulkano::command_buffer::DynamicState;
use vulkano::pipeline::viewport::Viewport;
use vulkano::buffer::CpuAccessibleBuffer;

pub struct Graphics {
    pub device: Arc<Device>,
    pub queue: Arc<Queue>,
    pub swapchain: Arc<Swapchain<Window>>,
    pub images: Vec<Arc<SwapchainImage<Window>>>,
    pub framebuffers: Vec<Arc<FramebufferAbstract + Send + Sync>>,
    // We only support a single pass for now.
    pub renderpass: Arc<RenderPassAbstract + Send + Sync>,
    pub dimensions: [u32; 2],
    // Should we always have a depth buffer?
    pub depth_buffer: Arc<AttachmentImage<D16Unorm>>,

    pub surface: Arc<Surface<Window>>,
    pub events_loop: EventsLoop,

    pub dynamic_state: DynamicState,

    pub sampler: Arc<Sampler>,

    // Frame-specific fields

}

impl Graphics {
    pub fn new() -> Graphics {
        let extensions = vulkano_win::required_extensions();
        let instance = vulkano::instance::Instance::new(None, &extensions, None)
            .expect("failed to create instance");

        let physical = vulkano::instance::PhysicalDevice::enumerate(&instance)
            .next().expect("no device available");
        println!("Using device: {} (type: {:?})", physical.name(), physical.ty());

        let events_loop = winit::EventsLoop::new();
        let surface = winit::WindowBuilder::new()
            .build_vk_surface(&events_loop, instance.clone()).unwrap();

        let dimensions;

        let queue = physical.queue_families().find(|&q| {
            q.supports_graphics() && surface.is_supported(q).unwrap_or(false)
        }).expect("couldn't find a graphical queue family");

        let device_ext = vulkano::device::DeviceExtensions {
            khr_swapchain: true,
            .. vulkano::device::DeviceExtensions::none()
        };

        let (device, mut queues) = vulkano::device::Device::new(
            physical,
            physical.supported_features(),
            &device_ext,
            [(queue, 0.5)].iter().cloned()).expect("failed to create device");
        let queue = queues.next().unwrap();

        let (swapchain, images) = {
            let caps = surface.capabilities(physical)
                .expect("failed to get surface capabilities");

            dimensions = caps.current_extent.unwrap_or([1024, 768]);

            let usage = caps.supported_usage_flags;
            let format = caps.supported_formats[0].0;
            let alpha = caps.supported_composite_alpha.iter().next().unwrap();

            vulkano::swapchain::Swapchain::new(
                device.clone(),
                surface.clone(),
                caps.min_image_count,
                format,
                dimensions,
                1,
                usage,
                &queue,
                vulkano::swapchain::SurfaceTransform::Identity,
                alpha,
                vulkano::swapchain::PresentMode::Fifo,
                true,
                None).expect("failed to create swapchain")
        };

        let renderpass = Arc::new(
            single_pass_renderpass!(device.clone(),
            attachments: {
                color: {
                    load: Clear,
                    store: Store,
                    format: swapchain.format(),
                    samples: 1,
                },
                depth: {
                    load: Clear,
                    store: DontCare,
                    format: vulkano::format::Format::D16Unorm,
                    samples: 1,
                }
            },
            pass: {
                color: [color],
                depth_stencil: {depth}
            }
        ).unwrap());

        let depth_buffer = vulkano::image::attachment::AttachmentImage::transient(
            device.clone(),
            dimensions,
            vulkano::format::D16Unorm,
        ).unwrap();

        let framebuffers = images.iter().map(|image| {
            let f: Arc<FramebufferAbstract + Send + Sync> = Arc::new(
                vulkano::framebuffer::Framebuffer::start(renderpass.clone())
                    .add(image.clone()).unwrap()
                    .add(depth_buffer.clone()).unwrap()
                    .build().unwrap());
            f
        }).collect::<Vec<_>>();

        let dynamic_state = vulkano::command_buffer::DynamicState {
            line_width: None,
            viewports: Some(vec![Viewport {
                origin: [0.0, 0.0],
                dimensions: [dimensions[0] as f32, dimensions[1] as f32],
                depth_range: 0.0 .. 1.0,
            }]),
            scissors: None,
        };

        let sampler = vulkano::sampler::Sampler::new(
            device.clone(),
            vulkano::sampler::Filter::Linear,
            vulkano::sampler::Filter::Linear,
            vulkano::sampler::MipmapMode::Nearest,
            vulkano::sampler::SamplerAddressMode::Repeat,
            vulkano::sampler::SamplerAddressMode::Repeat,
            vulkano::sampler::SamplerAddressMode::Repeat,
            0.0,
            1.0,
            0.0,
            0.0).unwrap();

        Graphics {
            device,
            queue,
            swapchain,
            images,
            framebuffers,
            renderpass,
            dimensions,
            surface,
            events_loop,
            depth_buffer,
            dynamic_state,
            sampler,
        }
    }

    pub fn recreate_swapchain(&mut self) -> bool {
        self.dimensions = self.surface.capabilities(self.physical_device())
            .expect("failed to get surface capabilities")
            .current_extent.unwrap_or([1024, 768]);

        let (new_swapchain, new_images) = match self.swapchain.recreate_with_dimension(self.dimensions) {
            Ok(r) => r,
            Err(vulkano::swapchain::SwapchainCreationError::UnsupportedDimensions) => {
                return false;
            },
            Err(err) => panic!("{:?}", err)
        };

        self.swapchain = new_swapchain;
        self.images = new_images;

        self.depth_buffer = vulkano::image::attachment::AttachmentImage::transient(self.device.clone(), self.dimensions, vulkano::format::D16Unorm).unwrap();

        self.dynamic_state.viewports = Some(vec![vulkano::pipeline::viewport::Viewport {
            origin: [0.0, 0.0],
            dimensions: [self.dimensions[0] as f32, self.dimensions[1] as f32],
            depth_range: 0.0 .. 1.0,
        }]);

        self.recreate_framebuffers();
        true
    }

    fn recreate_framebuffers(&mut self) {
        self.framebuffers = self.images.iter().map(|image| {
            let f: Arc<FramebufferAbstract + Send + Sync> = Arc::new(
                vulkano::framebuffer::Framebuffer::start(self.renderpass.clone())
                    .add(image.clone()).unwrap()
                    .add(self.depth_buffer.clone()).unwrap()
                    .build().unwrap());
            f
        }).collect::<Vec<_>>();
    }

    pub fn physical_device(&self) -> PhysicalDevice {
        self.device.physical_device()
    }

    pub fn load_model(&self, filename: &str) -> Arc<CpuAccessibleBuffer<[Vertex]>> {
        let model = ObjModel::from_file(filename);
         CpuAccessibleBuffer::from_iter(
             self.device.clone(),
             vulkano::buffer::BufferUsage::all(),
             model.vertices().iter().cloned()).expect("failed to create buffer")
    }

    pub fn load_texture(&self, filename: &str) -> (Arc<ImageViewAccess + Send + Sync>, Box<GpuFuture>) {
        let image = image::open(filename).unwrap().to_rgba();
        let image_width = image.width();
        let image_height = image.height();
        let image_data = image.into_raw().clone();
        let (tex, tex_future) = vulkano::image::immutable::ImmutableImage::from_iter(
            image_data.iter().cloned(),
            vulkano::image::Dimensions::Dim2d { width: image_width, height: image_height },
            vulkano::format::R8G8B8A8Unorm,
            self.queue.clone()).unwrap();
        (tex, Box::new(tex_future))
    }

    pub fn create_pipeline<V, F>(&self, vs: V, fs: F) -> Arc<GraphicsPipelineAbstract + Send + Sync>
        where V: GraphicsEntryPointAbstract<SpecializationConstants = ()>,
              V::PipelineLayout: Clone + 'static + Send + Sync,
              F: GraphicsEntryPointAbstract<SpecializationConstants = ()>,
              F::PipelineLayout: Clone + 'static + Send + Sync {
        let pipeline = Arc::new(vulkano::pipeline::GraphicsPipeline::start()
            .vertex_input_single_buffer::<Vertex>()
            .vertex_shader(vs, ())
            .triangle_list()
            .viewports_dynamic_scissors_irrelevant(1)
            .fragment_shader(fs, ())
            .depth_stencil_simple_depth()
            .render_pass(vulkano::framebuffer::Subpass::from(self.renderpass.clone(), 0).unwrap())
            .build(self.device.clone())
            .unwrap());
        pipeline
    }
}