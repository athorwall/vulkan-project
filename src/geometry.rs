use cgmath::*;

pub struct ModelVertex {
    pub position: Point3<f32>,
    pub normal: Vector3<f32>,
    pub uv: Point2<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vertex {
    pub position: (f32, f32, f32),
    pub normal: (f32, f32, f32),
    pub uv: (f32, f32),

    pub tangent_u: (f32, f32, f32),
    pub tangent_v: (f32, f32, f32),
}
impl_vertex!(Vertex, position, normal, uv, tangent_u, tangent_v);

pub fn compute_triangle(v0: ModelVertex, v1: ModelVertex, v2: ModelVertex) -> (Vertex, Vertex, Vertex) {
    let tangents_0 = compute_vertex_tangents(
        v0.normal,
        v1.position - v0.position,
        v1.uv - v0.uv,
        v2.position - v0.position,
        v2.uv - v0.uv,
    );
    let tangents_1 = compute_vertex_tangents(
        v1.normal,
        v0.position - v1.position,
        v0.uv - v1.uv,
        v2.position - v1.position,
        v2.uv - v1.uv,
    );
    let tangents_2 = compute_vertex_tangents(
        v2.normal,
        v0.position - v2.position,
        v0.uv - v2.uv,
        v1.position - v2.position,
        v1.uv - v2.uv,
    );
    (
        Vertex {
            position: (v0.position.x, v0.position.y, v0.position.z),
            normal: (v0.normal.x, v0.normal.y, v0.normal.z),
            uv: (v0.uv.x, v0.uv.y),
            tangent_u: (tangents_0.0.x, tangents_0.0.y, tangents_0.0.z),
            tangent_v: (tangents_0.1.x, tangents_0.1.y, tangents_0.1.z),
        },
        Vertex {
            position: (v1.position.x, v1.position.y, v1.position.z),
            normal: (v1.normal.x, v1.normal.y, v1.normal.z),
            uv: (v1.uv.x, v1.uv.y),
            tangent_u: (tangents_1.0.x, tangents_1.0.y, tangents_1.0.z),
            tangent_v: (tangents_1.1.x, tangents_1.1.y, tangents_1.1.z),
        },
        Vertex {
            position: (v2.position.x, v2.position.y, v2.position.z),
            normal: (v2.normal.x, v2.normal.y, v2.normal.z),
            uv: (v2.uv.x, v2.uv.y),
            tangent_u: (tangents_2.0.x, tangents_2.0.y, tangents_2.0.z),
            tangent_v: (tangents_2.1.x, tangents_2.1.y, tangents_2.1.z),
        },
    )
}

pub fn compute_vertex_tangents(
    normal: Vector3<f32>,
    edge_1: Vector3<f32>,
    uv_edge_1: Vector2<f32>,
    edge_2: Vector3<f32>,
    uv_edge_2: Vector2<f32>,
) -> (Vector3<f32>, Vector3<f32>) {
    let tangent_basis = Matrix3::from_cols(uv_edge_1.extend(0.0), uv_edge_2.extend(0.0), Vector3{ x: 0.0, y: 0.0, z: 1.0 });
    let world_basis = Matrix3::from_cols(edge_1, edge_2, normal);
    let from_tangent_space = world_basis * tangent_basis.invert().unwrap();
    let tangent_u = from_tangent_space * Vector3{ x: 1.0, y: 0.0, z: 0.0 };
    let tangent_v = from_tangent_space * Vector3{ x: 0.0, y: 1.0, z: 0.0 };
    (tangent_u, tangent_v)
}