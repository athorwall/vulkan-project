
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

pub struct Graphics {
    pub device: Arc<Device>,
    pub queue: Arc<Queue>,
    pub swapchain: Arc<Swapchain<Window>>,
    pub images: Vec<Arc<SwapchainImage<Window>>>,
    pub dimensions: [u32; 2],

    pub surface: Arc<Surface<Window>>,
    pub events_loop: EventsLoop,
}

impl Graphics {
    pub fn new() -> Graphics {
        let extensions = vulkano_win::required_extensions();
        let instance = vulkano::instance::Instance::new(None, &extensions, None).expect("failed to create instance");

        let physical = vulkano::instance::PhysicalDevice::enumerate(&instance)
            .next().expect("no device available");
        println!("Using device: {} (type: {:?})", physical.name(), physical.ty());

        let events_loop = winit::EventsLoop::new();
        let surface = winit::WindowBuilder::new().build_vk_surface(&events_loop, instance.clone()).unwrap();

        let dimensions;

        let queue = physical.queue_families().find(|&q| q.supports_graphics() &&
            surface.is_supported(q).unwrap_or(false))
            .expect("couldn't find a graphical queue family");

        let device_ext = vulkano::device::DeviceExtensions {
            khr_swapchain: true,
            .. vulkano::device::DeviceExtensions::none()
        };

        let (device, mut queues) = vulkano::device::Device::new(physical, physical.supported_features(),
                                                                &device_ext, [(queue, 0.5)].iter().cloned())
            .expect("failed to create device");
        let queue = queues.next().unwrap();

        let (swapchain, images) = {
            let caps = surface.capabilities(physical).expect("failed to get surface capabilities");

            dimensions = caps.current_extent.unwrap_or([1024, 768]);

            let usage = caps.supported_usage_flags;
            let format = caps.supported_formats[0].0;
            let alpha = caps.supported_composite_alpha.iter().next().unwrap();

            vulkano::swapchain::Swapchain::new(device.clone(), surface.clone(), caps.min_image_count, format, dimensions, 1,
                                               usage, &queue, vulkano::swapchain::SurfaceTransform::Identity,
                                               alpha,
                                               vulkano::swapchain::PresentMode::Fifo, true, None).expect("failed to create swapchain")
        };

        Graphics {
            device,
            queue,
            swapchain,
            images,
            dimensions,
            surface,
            events_loop,
        }
    }

    pub fn physical_device(&self) -> PhysicalDevice {
        self.device.physical_device()
    }

    pub fn load_model(&self, filename: &str) -> Vec<Vertex> {
        let monkey = ObjModel::from_file(filename);
        monkey.vertices()
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

    pub fn create_sampler(&self) -> Arc<Sampler> {
        vulkano::sampler::Sampler::new(
            self.device.clone(),
            vulkano::sampler::Filter::Linear,
            vulkano::sampler::Filter::Linear,
            vulkano::sampler::MipmapMode::Nearest,
            vulkano::sampler::SamplerAddressMode::Repeat,
            vulkano::sampler::SamplerAddressMode::Repeat,
            vulkano::sampler::SamplerAddressMode::Repeat,
            0.0,
            1.0,
            0.0,
            0.0,
        ).unwrap()
    }
}