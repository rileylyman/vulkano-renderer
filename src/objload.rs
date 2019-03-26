extern crate wavefront_obj;

use super::{IndexType, Vertex, Indices, Normal, TexVert};
use std::vec::Vec;
use wavefront_obj::obj::parse;
use wavefront_obj::obj::Primitive;

pub fn load_model(contents: &str) -> std::io::Result<(Vec<Vertex>, Vec<TexVert>, Vec<Normal>, Indices)> {
    let objs = parse(contents).expect("Could not parse obj file"); 

    println!("Loading obj file...");

    //TODO: Optimize. In place?
    let vertices: Vec<Vertex> = objs.objects[0].vertices.iter().cloned().map(|v| v.into()).collect::<_>();
    let normals: Vec<Normal> = objs.objects[0].normals.iter().cloned().map(|n| n.into()).collect::<_>();
    let tvertices: Vec<TexVert> = objs.objects[0].tex_vertices.iter().cloned().map(|vt| vt.into()).collect::<_>();

    //TODO: Support different shape types
    let mut v_idxs = Vec::new();
    let mut vn_idxs = Vec::new();
    let mut vt_idxs = Vec::new();
    for geometry in objs.objects[0].geometry.iter().cloned() {
        for shape in geometry.shapes.iter().cloned() {
           if let Primitive::Triangle(a, b, c) = shape.primitive {
               v_idxs.push(a.0 as IndexType); 
               v_idxs.push(b.0 as IndexType); 
               v_idxs.push(c.0 as IndexType); 
               if let (Some(at), Some(bt), Some(ct)) = (a.1, b.1, c.1) {
                   vt_idxs.push(at as IndexType); 
                   vt_idxs.push(bt as IndexType); 
                   vt_idxs.push(ct as IndexType); 
               } 
               if let (Some(an), Some(bn), Some(cn)) = (a.2, b.2, c.2) {
                   vn_idxs.push(an as IndexType); 
                   vn_idxs.push(bn as IndexType); 
                   vn_idxs.push(cn as IndexType); 
               } 
           }  
        }
    } 

    let indices = Indices {
        v: v_idxs,
        vn: vn_idxs,
        vt: vt_idxs,
    };

    println!("Finished loading obj");

    Ok((vertices, tvertices, normals, indices))
}

//TODO: 3d textures?
impl From<wavefront_obj::obj::TVertex> for TexVert {
   fn from(v: wavefront_obj::obj::TVertex) -> Self {
        TexVert {
           position2D: (v.u as f32, v.v as f32) 
        }  
   } 
} 

impl From<wavefront_obj::obj::Vertex> for Normal {
   fn from(v: wavefront_obj::obj::Vertex) -> Self {
        Normal {
           normal: (v.x as f32, v.y as f32, v.z as f32) 
        }  
   } 
} 

impl From<wavefront_obj::obj::Vertex> for Vertex {
   fn from(v: wavefront_obj::obj::Vertex) -> Self {
        Vertex {
           position: (v.x as f32, v.y as f32, v.z as f32) 
        }  
   } 
} 
