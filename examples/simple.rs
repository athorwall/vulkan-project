// Copyright (c) 2016 The vulkano developers
// Licensed under the Apache License, Version 2.0
// <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT
// license <LICENSE-MIT or http://opensource.org/licenses/MIT>,
// at your option. All files in the project carrying such
// notice may not be copied, modified, or distributed except
// according to those terms.

// For the purpose of this example all unused code is allowed.
#![allow(dead_code)]

extern crate cgmath;
extern crate winit;
extern crate time;
extern crate render;
#[macro_use]
extern crate vulkano;
#[macro_use]
extern crate vulkano_shader_derive;
extern crate vulkano_win;
extern crate image;

use cgmath::*;

use vulkano_win::VkSurfaceBuild;
use vulkano::sync::GpuFuture;
use vulkano::pipeline::shader::EntryPointAbstract;

use render::obj::*;

use std::sync::Arc;

use image::*;

fn load_model(filename: &str) -> Vec<Vertex> {
    let monkey = ObjModel::from_file(filename);
    monkey.vertices().iter().map(|vertex| {
        Vertex {
            position: vertex.position,
            normal: vertex.normal,
            uv: vertex.uv,
        }
    }).collect()
}

fn main() {
    let extensions = vulkano_win::required_extensions();
    let instance = vulkano::instance::Instance::new(None, &extensions, None).expect("failed to create instance");

    let physical = vulkano::instance::PhysicalDevice::enumerate(&instance)
        .next().expect("no device available");
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


    let mut depth_buffer = vulkano::image::attachment::AttachmentImage::transient(device.clone(), dimensions, vulkano::format::D16Unorm).unwrap();

    let monkey_vertices = load_model("resources/monkey.obj");
    let image = image::open("resources/Metal_Plate_007_COLOR.jpg").unwrap().to_rgba();
    let image_data = image.into_raw().clone();
    let (texture, texture_future) = vulkano::image::immutable::ImmutableImage::from_iter(
        image_data.iter().cloned(),
        vulkano::image::Dimensions::Dim2d { width: 1024, height: 1024 },
        vulkano::format::R8G8B8A8Srgb,
        queue.clone()).unwrap();
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
        0.0,
    ).unwrap();

    let vertex_buffer = vulkano::buffer::cpu_access::CpuAccessibleBuffer
    ::from_iter(device.clone(), vulkano::buffer::BufferUsage::all(), monkey_vertices.iter().cloned()).expect("failed to create buffer");

    let mut proj = cgmath::perspective(cgmath::Rad(std::f32::consts::FRAC_PI_2), { dimensions[0] as f32 / dimensions[1] as f32 }, 0.01, 100.0);
    let camera = cgmath::Matrix4::look_at(Point3 { x: 0.0, y: 0.4, z: 2.0 }, Point3 { x: 0.0, y: 0.0, z: 0.0 }, Vector3 { x: 0.0, y: -1.0, z: 0.0 }).invert().unwrap();

    let uniform_buffer = vulkano::buffer::cpu_pool::CpuBufferPool::<vs::ty::Data>
    ::new(device.clone(), vulkano::buffer::BufferUsage::all());

    let vs = vs::Shader::load(device.clone()).expect("failed to create shader module");
    let fs = fs::Shader::load(device.clone()).expect("failed to create shader module");

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
        ).unwrap()
    );

    let pipeline = Arc::new(vulkano::pipeline::GraphicsPipeline::start()
        .vertex_input_single_buffer()
        .vertex_shader(vs.main_entry_point(), ())
        .triangle_list()
        .viewports_dynamic_scissors_irrelevant(1)
        .fragment_shader(fs.main_entry_point(), ())
        .depth_stencil_simple_depth()
        .render_pass(vulkano::framebuffer::Subpass::from(renderpass.clone(), 0).unwrap())
        .build(device.clone())
        .unwrap());
    let mut framebuffers: Option<Vec<Arc<vulkano::framebuffer::Framebuffer<_,_>>>> = None;

    //println!("{:?}", fs.main_entry_point().layout());

    let sampler_set: Arc<vulkano::descriptor::DescriptorSet+ Send + Sync> = Arc::new(vulkano::descriptor::descriptor_set::PersistentDescriptorSet::start(pipeline.clone(), 0)
        .add_sampled_image(texture.clone(), sampler.clone()).expect("Failed to add sampled image")
        .build().expect("Failed to build sampler set")
    );

    let mut pool = vulkano::descriptor::descriptor_set::FixedSizeDescriptorSetsPool::new(pipeline.clone(), 1);

    let mut recreate_swapchain = false;

    // TODO: understand this!
    let mut device_future = Box::new(vulkano::sync::now(device.clone())) as Box<GpuFuture>;
    let mut tex_future = Box::new(texture_future) as Box<GpuFuture>;
    let mut previous_frame: Box<GpuFuture> = Box::new(device_future.join(tex_future));


    let rotation_start = std::time::Instant::now();

    let mut dynamic_state = vulkano::command_buffer::DynamicState {
        line_width: None,
        viewports: Some(vec![vulkano::pipeline::viewport::Viewport {
            origin: [0.0, 0.0],
            dimensions: [dimensions[0] as f32, dimensions[1] as f32],
            depth_range: 0.0 .. 1.0,
        }]),
        scissors: None,
    };

    loop {
        previous_frame.cleanup_finished();

        if recreate_swapchain {

            dimensions = surface.capabilities(physical)
                .expect("failed to get surface capabilities")
                .current_extent.unwrap_or([1024, 768]);

            let (new_swapchain, new_images) = match swapchain.recreate_with_dimension(dimensions) {
                Ok(r) => r,
                Err(vulkano::swapchain::SwapchainCreationError::UnsupportedDimensions) => {
                    continue;
                },
                Err(err) => panic!("{:?}", err)
            };

            swapchain = new_swapchain;
            images = new_images;

            depth_buffer = vulkano::image::attachment::AttachmentImage::transient(device.clone(), dimensions, vulkano::format::D16Unorm).unwrap();

            framebuffers = None;

            proj = cgmath::perspective(cgmath::Rad(std::f32::consts::FRAC_PI_2), { dimensions[0] as f32 / dimensions[1] as f32 }, 0.01, 100.0);

            dynamic_state.viewports = Some(vec![vulkano::pipeline::viewport::Viewport {
                origin: [0.0, 0.0],
                dimensions: [dimensions[0] as f32, dimensions[1] as f32],
                depth_range: 0.0 .. 1.0,
            }]);

            recreate_swapchain = false;
        }

        if framebuffers.is_none() {
            framebuffers = Some(images.iter().map(|image| {
                Arc::new(vulkano::framebuffer::Framebuffer::start(renderpass.clone())
                    .add(image.clone()).unwrap()
                    .add(depth_buffer.clone()).unwrap()
                    .build().unwrap())
            }).collect::<Vec<_>>());
        }

        let uniform_buffer_subbuffer = {
            let elapsed = rotation_start.elapsed();
            let rotation = elapsed.as_secs() as f64 + elapsed.subsec_nanos() as f64 / 1_000_000_000.0;
            let rotation = cgmath::Matrix3::from_angle_y(cgmath::Rad(rotation as f32 / 2.0));

            let uniform_data = vs::ty::Data {
                world : cgmath::Matrix4::from(rotation).into(),
                view : camera.invert().unwrap().into(),
                proj : proj.into(),
            };

            uniform_buffer.next(uniform_data).unwrap()
        };

        let set: Arc<vulkano::descriptor::DescriptorSet + Send + Sync> = Arc::from(pool.next()
            .add_buffer(uniform_buffer_subbuffer).unwrap()
            .build().unwrap());

        let (image_num, acquire_future) = match vulkano::swapchain::acquire_next_image(swapchain.clone(),
                                                                                       None) {
            Ok(r) => r,
            Err(vulkano::swapchain::AcquireError::OutOfDate) => {
                recreate_swapchain = true;
                continue;
            },
            Err(err) => panic!("{:?}", err)
        };

        let command_buffer = vulkano::command_buffer::AutoCommandBufferBuilder::primary_one_time_submit(device.clone(), queue.family()).unwrap()
            .begin_render_pass(
                framebuffers.as_ref().unwrap()[image_num].clone(), false,
                vec![
                    [0.0, 0.0, 1.0, 1.0].into(),
                    1f32.into()
                ]).unwrap()
            .draw(
                pipeline.clone(),
                &dynamic_state,
                vertex_buffer.clone(),
                (sampler_set.clone(), set.clone()),
                ()).unwrap()
            .end_render_pass().unwrap()
            .build().unwrap();

        let future = previous_frame.join(acquire_future)
            .then_execute(queue.clone(), command_buffer).unwrap()
            .then_swapchain_present(queue.clone(), swapchain.clone(), image_num)
            .then_signal_fence_and_flush();

        match future {
            Ok(future) => {
                previous_frame = Box::new(future) as Box<_>;
            }
            Err(vulkano::sync::FlushError::OutOfDate) => {
                recreate_swapchain = true;
                previous_frame = Box::new(vulkano::sync::now(device.clone())) as Box<_>;
            }
            Err(e) => {
                println!("{:?}", e);
                previous_frame = Box::new(vulkano::sync::now(device.clone())) as Box<_>;
            }
        }

        let mut done = false;
        events_loop.poll_events(|ev| {
            match ev {
                winit::Event::WindowEvent { event: winit::WindowEvent::CloseRequested, .. } => done = true,
                _ => ()
            }
        });
        if done { return; }
    }
}

#[derive(Copy, Clone)]
pub struct Vertex {
    position: (f32, f32, f32),
    normal: (f32, f32, f32),
    uv: (f32, f32),
}

impl_vertex!(Vertex, position, normal, uv);

mod vs {
    #[derive(VulkanoShader)]
    #[ty = "vertex"]
    #[src = "
#version 450

layout(location = 0) in vec3 position;
layout(location = 1) in vec3 normal;
layout(location = 2) in vec2 uv;

// in world space
layout(location = 0) out vec3 v_normal;
layout(location = 1) out vec2 v_uv;
layout(location = 2) out vec3 v_world_pos;
layout(location = 3) out vec3 v_view_pos;

layout(set = 1, binding = 0) uniform Data {
    mat4 world;
    mat4 view;
    mat4 proj;
} uniforms;

void main() {
    mat4 worldview = uniforms.view * uniforms.world;
    v_normal = transpose(inverse(mat3(uniforms.world))) * normal;
    v_uv = uv;
    gl_Position = uniforms.proj * worldview * vec4(position, 1.0);
    v_view_pos = (worldview * vec4(position, 1.0)).xyz;
    v_world_pos = (uniforms.world * vec4(position, 1.0)).xyz;
}
"]
    struct Dummy;
}

mod fs {
    #[derive(VulkanoShader)]
    #[ty = "fragment"]
    #[src = "
#version 450

layout(location = 0) in vec3 v_normal;
layout(location = 1) in vec2 v_uv;
layout(location = 2) in vec3 v_world_pos;
layout(location = 3) in vec3 v_view_pos;

layout(location = 0) out vec4 f_color;

layout(set = 0, binding = 0) uniform sampler2D color;
//layout(set = 1, binding = 0) uniform sampler2D normal;

const vec3 LIGHT = vec3(0.0, 0.0, 1.0);
const vec3 POINT_LIGHT_POSITION = vec3(1.0, 1.0, 4.0);
const vec3 POINT_LIGHT_INTENSITY = vec3(10.0, 10.0, 10.0);
const vec3 AMBIENT_LIGHT = vec3(0.1, 0.1, 0.1);

const float LAMBERT_COEFFICIENT = 1.0;
const float SPECULAR_COEFFICIENT = 1.0;

const float ROUGHNESS = 0.04;
const float REFRACTION = 0.1;

const vec4 MATERIAL_COLOR = vec4(1.0, 0.0, 0.0, 1.0);

float schlick(vec3 v, vec3 h, float refraction) {
    float r0_sqrt = (1 - refraction) / (1 + refraction);
    float r0 = r0_sqrt * r0_sqrt;
    return r0 + (1 - r0) * pow(1 - dot(v, h), 5);
}

// all vec arguments must be normalized
float geometric_attenuation(vec3 v, vec3 n, vec3 h, vec3 l) {
    float vh = dot(v, h);
    float hn = dot(h, n);
    float ln = dot(l, n);
    float vn = dot(v, n);
    return min(1, min(2 * hn * vn / vh, 2 * hn * ln / vh));
}

float ggx_chi(float x) {
    return x > 0 ? 1 : 0;
}

// all vec arguments must be normalized
float ggx_distribution(vec3 h, vec3 n, float roughness) {
    float r2 = roughness * roughness;
    float nh = dot(n, h);
    float denom = nh * nh * r2 + (1 - nh * nh);
    return r2 * ggx_chi(nh) / (3.14 * denom * denom);
}

// all vec arguments must be normalized
float cook_torrance(vec3 v, vec3 n, vec3 l, float refraction, float roughness) {
    vec3 h = normalize((v + l) / 2);
    float d = ggx_distribution(h, n, roughness);
    float g = geometric_attenuation(v, n, h, l);
    float s = schlick(v, h, refraction);

    return d * g * s / (4 * dot(v, n) * dot(n, l));
}

void main() {
    vec3 l = POINT_LIGHT_POSITION - v_world_pos;
    float d2 = dot(l, l);
    vec3 v = -v_view_pos;

    vec4 lambert_component = MATERIAL_COLOR;
    vec4 lambert = LAMBERT_COEFFICIENT * lambert_component;

    float specular_component = cook_torrance(normalize(v), normalize(v_normal), normalize(l), REFRACTION, ROUGHNESS);
    vec4 specular = (SPECULAR_COEFFICIENT * specular_component).xxxx;

    vec4 brdf_value = lambert + specular;

    float c = max(dot(normalize(v_normal), normalize(l)), 0.0);
    vec4 irradiance = vec4(POINT_LIGHT_INTENSITY / d2, 1.0);
    vec4 lighting_color = irradiance * c.xxxx * brdf_value;
    //vec4 texture_color = texture(color, v_uv);
    vec4 texture_color = vec4(1.0, 1.0, 1.0, 1.0);
    f_color = texture_color * lighting_color;
}
"]
    struct Dummy;
}