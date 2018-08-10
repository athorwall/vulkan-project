use std::env;
use std::fs::File;
use std::io::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vertex {
    pub position: (f32, f32, f32),
    pub normal: (f32, f32, f32),
    pub uv: (f32, f32),
}

#[derive(PartialEq, Debug)]
pub struct ObjModel {
    /// Vertex coordinates.
    pub v: Vec<(f32, f32, f32, f32)>,

    /// Vertex texture coordinates.
    pub vt: Vec<(f32, f32)>,

    /// Vertex normals.
    pub vn: Vec<(f32, f32, f32)>,

    /// Faces.
    pub f: Vec<Vec<VertexIndices>>,
}

impl ObjModel {
    pub fn new() -> ObjModel {
        ObjModel {
            v: vec![],
            vt: vec![],
            vn: vec![],
            f: vec![],
        }
    }

    pub fn from_file(filename: &str) -> Self {
        let mut f = File::open(filename).expect("File not found!");
        let mut contents = String::new();
        f.read_to_string(&mut contents).expect("Error reading file!");
        ObjModel::parse(contents.as_str())
    }

    pub fn parse(input: &str) -> Self {
        let mut model = ObjModel::new();
        for line in input.lines() {
            match ObjModel::parse_line(line) {
                Some(ObjModelLine::VertexPos(x, y, z, w)) => {
                    model.v.push((x, y, z, w));
                },
                Some(ObjModelLine::VertexNormal(x, y, z)) => {
                    model.vn.push((x, y, z));
                },
                Some(ObjModelLine::VertexUv(u, v)) => {
                    model.vt.push((u, v));
                },
                Some(ObjModelLine::Face(face)) => {
                    model.f.push(face);
                },
                _ => {}
            }
        }
        model
    }

    fn parse_line(line: &str) -> Option<ObjModelLine> {
        let mut parts = line.split_whitespace();
        match parts.next() {
            Some("v") => {
                let float_parts: Vec<f32> = parts.map(|part| {
                    part.parse().expect("Couldn't parse float!")
                }).collect();
                match (float_parts.get(0), float_parts.get(1), float_parts.get(2), float_parts.get(3)) {
                    (Some(x), Some(y), Some(z), Some(w)) => {
                        Some(ObjModelLine::VertexPos(*x, *y, *z, *w))
                    },
                    (Some(x), Some(y), Some(z), None) => {
                        Some(ObjModelLine::VertexPos(*x, *y, *z, 1.0))
                    },
                    _ => {
                        // TODO: Better error handling
                        panic!("Invalid vertex!");
                    },
                }
            },
            Some("vt") => {
                let float_parts: Vec<f32> = parts.map(|part| {
                    part.parse().expect("Couldn't parse float!")
                }).collect();
                match (float_parts.get(0), float_parts.get(1)) {
                    (Some(u), Some(v)) => {
                        Some(ObjModelLine::VertexUv(*u, *v))
                    },
                    _ => {
                        // TODO: Better error handling
                        panic!("Invalid texture coordinates!");
                    },
                }
            },
            Some("vn") => {
                let float_parts: Vec<f32> = parts.map(|part| {
                    part.parse().expect("Couldn't parse float!")
                }).collect();
                match (float_parts.get(0), float_parts.get(1), float_parts.get(2)) {
                    (Some(x), Some(y), Some(z)) => {
                        Some(ObjModelLine::VertexNormal(*x, *y, *z))
                    },
                    _ => {
                        // TODO: Better error handling
                        panic!("Invalid vertex!");
                    },
                }
            },
            Some("f") => {
                let indices_parts: Vec<VertexIndices> = parts.map(|part| {
                    ObjModel::parse_vertex_indices(part)
                }).collect();
                Some(ObjModelLine::Face(indices_parts))
            },
            _ => None,
        }
    }

    fn parse_vertex_indices(input: &str) -> VertexIndices {
        let parts: Vec<Option<usize>> = input.split('/').map(|part| {
            if part.len() > 0 {
                Some(part.parse().expect("Couldn't parse usize!"))
            } else {
                None
            }
        }).collect();
        VertexIndices {
            v: parts.get(0).unwrap().unwrap(),
            vt: *parts.get(1).unwrap(),
            vn: parts.get(2).unwrap().unwrap(),
        }
    }

    pub fn vertices(&self) -> Vec<Vertex> {
        let mut verts = vec![];
        for face in &self.f {
            match face.len() {
                3 => {
                    verts.push(self.lookup_vertex_indices(face[0]));
                    verts.push(self.lookup_vertex_indices(face[1]));
                    verts.push(self.lookup_vertex_indices(face[2]));
                },
                4 => {
                    verts.push(self.lookup_vertex_indices(face[0]));
                    verts.push(self.lookup_vertex_indices(face[1]));
                    verts.push(self.lookup_vertex_indices(face[2]));
                    verts.push(self.lookup_vertex_indices(face[2]));
                    verts.push(self.lookup_vertex_indices(face[3]));
                    verts.push(self.lookup_vertex_indices(face[0]));
                },
                _ => panic!("Invalid face!"),
            }
        }
        verts
    }

    fn lookup_vertex_indices(&self, indices: VertexIndices) -> Vertex {
        let pos = self.v.get(indices.v - 1).unwrap();
        let normal = self.vn.get(indices.vn - 1).unwrap();
        let uv = indices.vt
            .map(|i| { self.vt.get(i - 1).unwrap() })
            .unwrap_or(&(0.0, 0.0));
        Vertex {
            position: (pos.0, pos.1, pos.2),
            normal: *normal,
            uv: *uv,
        }
    }
}

#[derive(PartialEq, Debug)]
enum ObjModelLine {
    VertexPos(f32, f32, f32, f32),
    VertexUv(f32, f32),
    VertexNormal(f32, f32, f32),
    Face(Vec<VertexIndices>),
}

/// Describes a vertex via the indices of its properties in the OBJ file.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct VertexIndices {
    /// Index of the vertex's coordinates.
    v: usize,

    /// Optional index of the vertex's texture coordinates.
    vt: Option<usize>,

    /// Index of the vertex's normals.
    vn: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_line() {
        assert_eq!(ObjModel::parse_line("v 1.0 2.0 3.0"), Some(ObjModelLine::VertexPos(1.0, 2.0, 3.0, 1.0)));
        assert_eq!(ObjModel::parse_line("v 1.0 2.0 3.0 4.0"), Some(ObjModelLine::VertexPos(1.0, 2.0, 3.0, 4.0)));

        assert_eq!(ObjModel::parse_line("vt 1.0 2.0"), Some(ObjModelLine::VertexUv(1.0, 2.0)));

        assert_eq!(ObjModel::parse_line("vn 1.0 2.0 3.0"), Some(ObjModelLine::VertexNormal(1.0, 2.0, 3.0)));

        assert_eq!(ObjModel::parse_line("f 1/2/3 4/5/6 7/8/9"), Some(ObjModelLine::Face(vec![
            VertexIndices { v: 1, vt: Some(2), vn: 3 },
            VertexIndices { v: 4, vt: Some(5), vn: 6 },
            VertexIndices { v: 7, vt: Some(8), vn: 9 },
        ])));
        assert_eq!(ObjModel::parse_line("f 1//3 4//6 7//9"), Some(ObjModelLine::Face(vec![
            VertexIndices { v: 1, vt: None, vn: 3 },
            VertexIndices { v: 4, vt: None, vn: 6 },
            VertexIndices { v: 7, vt: None, vn: 9 },
        ])));
    }

    #[test]
    fn test_parse_1() {
        let s = r#"# obj file
v 1.0 2.0 3.0 4.0
vt 1.0 2.0
vn 1.0 2.0 3.0
f 0/0/0 1/1/1 2/2/2
"#;
        let actual = ObjModel::parse(s);
        let expected = ObjModel {
            v: vec![
                (1.0, 2.0, 3.0, 4.0),
            ],
            vn: vec![
                (1.0, 2.0, 3.0),
            ],
            vt: vec![
                (1.0, 2.0),
            ],
            f: vec![
                vec![
                    VertexIndices { v: 0, vt: Some(0), vn: 0 },
                    VertexIndices { v: 1, vt: Some(1), vn: 1 },
                    VertexIndices { v: 2, vt: Some(2), vn: 2 },
                ],
            ],
        };
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_parse_2() {
        let s = r#"# Blender v2.76 (sub 0) OBJ File: ''
# www.blender.org
mtllib monkey.mtl
o Suzanne
v 1.0 1.0 1.0
v -1.0 1.0 1.0
vn 0.5 -0.5 0.5
vn 0.5 0.5 0.5
usemtl None
s 1
f 7830//1 516//2 3//3 517//4
f 7821//5 528//6 48//7 529//8
"#;
        let actual = ObjModel::parse(s);
        let expected = ObjModel {
            v: vec![
                (1.0, 1.0, 1.0, 1.0),
                (-1.0, 1.0, 1.0, 1.0),
            ],
            vn: vec![
                (0.5, -0.5, 0.5),
                (0.5, 0.5, 0.5),
            ],
            vt: vec![],
            f: vec![
                vec![
                    VertexIndices { v: 7830, vt: None, vn: 1 },
                    VertexIndices { v: 516, vt: None, vn: 2 },
                    VertexIndices { v: 3, vt: None, vn: 3 },
                    VertexIndices { v: 517, vt: None, vn: 4 },
                ],
                vec![
                    VertexIndices { v: 7821, vt: None, vn: 5 },
                    VertexIndices { v: 528, vt: None, vn: 6 },
                    VertexIndices { v: 48, vt: None, vn: 7 },
                    VertexIndices { v: 529, vt: None, vn: 8 },
                ],
            ],
        };
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_vertices() {
        let obj = ObjModel {
            v: vec![
                (1.0, 1.0, 1.0, 1.0),
                (-1.0, 1.0, 1.0, 1.0),
                (-1.0, -1.0, 1.0, 1.0),
                (1.0, -1.0, 1.0, 1.0),
            ],
            vn: vec![
                (0.5, -0.5, 0.5),
            ],
            vt: vec![
                (0.0, 0.0),
                (1.0, 1.0),
            ],
            f: vec![
                vec![
                    VertexIndices { v: 1, vt: Some(1), vn: 1 },
                    VertexIndices { v: 2, vt: Some(1), vn: 1 },
                    VertexIndices { v: 3, vt: Some(2), vn: 1 },
                    VertexIndices { v: 4, vt: Some(2), vn: 1 },
                ],
                vec![
                    VertexIndices { v: 1, vt: Some(1), vn: 1 },
                    VertexIndices { v: 2, vt: Some(1), vn: 1 },
                    VertexIndices { v: 3, vt: Some(2), vn: 1 },
                ],
            ],
        };
        let actual = obj.vertices();
        let expected = vec![
            Vertex {
                position: (1.0, 1.0, 1.0),
                normal: (0.5, -0.5, 0.5),
                uv: (0.0, 0.0),
            },
            Vertex {
                position: (-1.0, 1.0, 1.0),
                normal: (0.5, -0.5, 0.5),
                uv: (0.0, 0.0),
            },
            Vertex {
                position: (-1.0, -1.0, 1.0),
                normal: (0.5, -0.5, 0.5),
                uv: (1.0, 1.0),
            },
            Vertex {
                position: (-1.0, -1.0, 1.0),
                normal: (0.5, -0.5, 0.5),
                uv: (1.0, 1.0),
            },
            Vertex {
                position: (1.0, -1.0, 1.0),
                normal: (0.5, -0.5, 0.5),
                uv: (1.0, 1.0),
            },
            Vertex {
                position: (1.0, 1.0, 1.0),
                normal: (0.5, -0.5, 0.5),
                uv: (0.0, 0.0),
            },
            Vertex {
                position: (1.0, 1.0, 1.0),
                normal: (0.5, -0.5, 0.5),
                uv: (0.0, 0.0),
            },
            Vertex {
                position: (-1.0, 1.0, 1.0),
                normal: (0.5, -0.5, 0.5),
                uv: (0.0, 0.0),
            },
            Vertex {
                position: (-1.0, -1.0, 1.0),
                normal: (0.5, -0.5, 0.5),
                uv: (1.0, 1.0),
            },
        ];
        assert_eq!(actual, expected);
    }
}
