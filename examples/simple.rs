#![allow(dead_code)]

extern crate cgmath;
extern crate winit;
extern crate time;
extern crate render;
extern crate vulkano;
#[macro_use]
extern crate vulkano_shader_derive;
extern crate vulkano_win;
extern crate image;

use std::sync::Arc;

use cgmath::*;

use vulkano::descriptor::DescriptorSet;
use vulkano::sync::GpuFuture;

use render::graphics::*;

fn main() {
    let mut graphics = Graphics::new();

    let model = graphics.load_model("resources/sphere.obj");
    let (texture, texture_future) = graphics.load_texture("resources/Metal_Plate_007_COLOR.png");
    let (normal_map, normal_map_future) = graphics.load_texture("resources/Metal_Plate_007_NORM.png");

    let mut proj = cgmath::perspective(
        cgmath::Rad(std::f32::consts::FRAC_PI_2),
        { graphics.dimensions[0] as f32 / graphics.dimensions[1] as f32 },
        0.01,
        100.0);
    let camera = cgmath::Matrix4::look_at(
        Point3 { x: 0.0, y: 0.4, z: 2.0 },
        Point3 { x: 0.0, y: 0.0, z: 0.0 },
        Vector3 { x: 0.0, y: -1.0, z: 0.0 }).invert().unwrap();

    let uniform_buffer = vulkano::buffer::cpu_pool::CpuBufferPool::<vs::ty::Data>::new(
        graphics.device.clone(),
        vulkano::buffer::BufferUsage::all());

    let vs = vs::Shader::load(graphics.device.clone()).expect("failed to create shader module");
    let fs = fs::Shader::load(graphics.device.clone()).expect("failed to create shader module");

    let pipeline = graphics.create_pipeline(vs.main_entry_point(), fs.main_entry_point());

   let sampler_set: Arc<DescriptorSet + Send + Sync> = Arc::new(vulkano::descriptor::descriptor_set::PersistentDescriptorSet::start(pipeline.clone(), 0)
        .add_sampled_image(texture.clone(), graphics.sampler.clone()).expect("Failed to add sampled image")
        .add_sampled_image(normal_map.clone(), graphics.sampler.clone()).expect("Failed to load normal map!")
        .build().expect("Failed to build sampler set")
    );

    let mut pool = vulkano::descriptor::descriptor_set::FixedSizeDescriptorSetsPool::new(pipeline.clone(), 1);

    let mut recreate_swapchain = false;

    let device_future = Box::new(vulkano::sync::now(graphics.device.clone())) as Box<GpuFuture>;
    let mut previous_frame: Box<GpuFuture> = Box::new(device_future.join(texture_future).join(normal_map_future));

    let rotation_start = std::time::Instant::now();

    loop {
        previous_frame.cleanup_finished();

        if recreate_swapchain {
            if !graphics.recreate_swapchain() {
                continue;
            }
            proj = cgmath::perspective(cgmath::Rad(std::f32::consts::FRAC_PI_2), { graphics.dimensions[0] as f32 / graphics.dimensions[1] as f32 }, 0.01, 100.0);
            recreate_swapchain = false;
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

        let set: Arc<DescriptorSet + Send + Sync> = Arc::from(pool.next()
            .add_buffer(uniform_buffer_subbuffer).unwrap()
            .build().unwrap());

        let (image_num, acquire_future) = match vulkano::swapchain::acquire_next_image(graphics.swapchain.clone(),
                                                                                       None) {
            Ok(r) => r,
            Err(vulkano::swapchain::AcquireError::OutOfDate) => {
                recreate_swapchain = true;
                continue;
            },
            Err(err) => panic!("{:?}", err)
        };

        let command_buffer = vulkano::command_buffer::AutoCommandBufferBuilder::primary_one_time_submit(graphics.device.clone(), graphics.queue.family()).unwrap()
            .begin_render_pass(
                graphics.framebuffers[image_num].clone(), false,
                vec![
                    [0.0, 0.0, 1.0, 1.0].into(),
                    1f32.into()
                ]).unwrap()
            .draw(
                pipeline.clone(),
                &graphics.dynamic_state,
                vec![model.clone()],
                (sampler_set.clone(), set.clone()),
                ()).unwrap()
            .end_render_pass().unwrap()
            .build().unwrap();

        let future = previous_frame.join(acquire_future)
            .then_execute(graphics.queue.clone(), command_buffer).unwrap()
            .then_swapchain_present(graphics.queue.clone(), graphics.swapchain.clone(), image_num)
            .then_signal_fence_and_flush();

        match future {
            Ok(future) => {
                previous_frame = Box::new(future) as Box<_>;
            }
            Err(vulkano::sync::FlushError::OutOfDate) => {
                recreate_swapchain = true;
                previous_frame = Box::new(vulkano::sync::now(graphics.device.clone())) as Box<_>;
            }
            Err(e) => {
                println!("{:?}", e);
                previous_frame = Box::new(vulkano::sync::now(graphics.device.clone())) as Box<_>;
            }
        }

        let mut done = false;
        graphics.events_loop.poll_events(|ev| {
            match ev {
                winit::Event::WindowEvent { event: winit::WindowEvent::CloseRequested, .. } => done = true,
                _ => ()
            }
        });
        if done { return; }
    }
}

mod vs {
    #[derive(VulkanoShader)]
    #[ty = "vertex"]
    #[src = "
#version 450

layout(location = 0) in vec3 position;
layout(location = 1) in vec3 normal;
layout(location = 2) in vec2 uv;
layout(location = 3) in vec3 tangent_u;
layout(location = 4) in vec3 tangent_v;

layout(location = 0) out vec3 v_world_normal;
layout(location = 1) out vec2 v_uv;
layout(location = 2) out vec3 v_world_pos;
layout(location = 3) out vec3 v_view_pos;
layout(location = 4) out vec3 v_world_tangent_u;
layout(location = 5) out vec3 v_world_tangent_v;

layout(set = 1, binding = 0) uniform Data {
    mat4 world;
    mat4 view;
    mat4 proj;
} uniforms;

void main() {
    mat4 worldview = uniforms.view * uniforms.world;
    v_world_normal = transpose(inverse(mat3(uniforms.world))) * normal;
    v_world_tangent_u = transpose(inverse(mat3(uniforms.world))) * tangent_u;
    v_world_tangent_v = transpose(inverse(mat3(uniforms.world))) * tangent_v;
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

layout(location = 0) in vec3 v_world_normal;
layout(location = 1) in vec2 v_uv;
layout(location = 2) in vec3 v_world_pos;
layout(location = 3) in vec3 v_view_pos;
layout(location = 4) in vec3 v_world_tangent_u;
layout(location = 5) in vec3 v_world_tangent_v;

layout(location = 0) out vec4 f_color;

layout(set = 0, binding = 0) uniform sampler2D color;
layout(set = 0, binding = 1) uniform sampler2D normal;

const vec3 LIGHT = vec3(0.0, 0.0, 1.0);
const vec3 POINT_LIGHT_POSITION = vec3(1.0, 1.0, 4.0);
const vec3 POINT_LIGHT_INTENSITY = vec3(10.0, 10.0, 10.0);
const vec3 AMBIENT_LIGHT = vec3(0.1, 0.1, 0.1);

const float LAMBERT_COEFFICIENT = 1.0;
const float SPECULAR_COEFFICIENT = 1.0;

const float ROUGHNESS = 0.12;
const float REFRACTION = 0.5;

const vec4 MATERIAL_COLOR = vec4(1.0, 1.0, 1.0, 1.0);

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

vec4 point_light(vec3 pos, vec3 intensity, vec3 normal) {
    vec3 l = pos - v_world_pos;
    float d2 = dot(l, l);
    vec3 v = -v_view_pos;

    vec4 lambert_component = MATERIAL_COLOR;
    vec4 lambert = LAMBERT_COEFFICIENT * lambert_component;

    float specular_component = cook_torrance(normalize(v), normalize(normal), normalize(l), REFRACTION, ROUGHNESS);
    vec4 specular = (SPECULAR_COEFFICIENT * specular_component).xxxx;

    vec4 brdf_value = lambert + specular;

    float c = max(dot(normalize(normal), normalize(l)), 0.0);
    vec4 irradiance = vec4(intensity / d2, 1.0);
    return irradiance * c.xxxx * brdf_value;
}

void main() {
    vec4 normal_map_color = texture(normal, v_uv);
    vec4 normals = normal_map_color * 2.0 - vec4(1.0, 1.0, 1.0, 1.0);
    vec3 adjusted_normal = normals.x * normalize(v_world_tangent_u) + normals.y * normalize(v_world_tangent_v) + normals.z * normalize(v_world_normal);
    adjusted_normal = normalize(adjusted_normal);
    vec4 lighting_color = point_light(POINT_LIGHT_POSITION, POINT_LIGHT_INTENSITY, adjusted_normal);
    vec4 texture_color = texture(color, v_uv);
    f_color = texture_color * lighting_color;
}
"]
    struct Dummy;
}