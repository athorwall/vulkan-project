
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

pub struct Graphics {
    pub device: Arc<Device>,
    pub queue: Arc<Queue>,
    pub physical_device_index: usize,
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
        let physical_device_index = physical.index();
        println!("Using device: {} (type: {:?})", physical.name(), physical.ty());

        let mut events_loop = winit::EventsLoop::new();
        let surface = winit::WindowBuilder::new().build_vk_surface(&events_loop, instance.clone()).unwrap();

        let mut dimensions;

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

        let (mut swapchain, mut images) = {
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
            physical_device_index,
            swapchain,
            images,
            dimensions,
            surface,
            events_loop,
        }
    }
}