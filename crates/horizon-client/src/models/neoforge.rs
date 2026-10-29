//! NeoForge's model loaders, which mods' block models name with `"loader"`:
//! `neoforge:composite`, its `children` baked together as one model, and
//! `neoforge:obj`, a Wavefront OBJ file (`"model"`) whose faces are the
//! model's quads, textured through its `.mtl`'s `map_Kd` (a `#slot` of the
//! model's textures, or a texture id). Vanilla's models never have a loader;
//! this reads only those that do.
use crate::{Result, model_library::ParsedModels};
use horizon_assets::{
    bake::{BlockModelTransform, ModelQuad, bake_block_model},
    model::{CuboidModel, Direction, Material},
    resolve::ResolvedCuboidModel,
};
use serde_json::Value;
use std::{fs, path::Path};

/// The JSON naming the NeoForge loader model `name` draws with: its own,
/// or, as NeoForge takes a parent's geometry when a model has none, its
/// nearest parent's (a red valve handle, its textures over a valve
/// handle's OBJ). None for a model with elements or a vanilla parent first.
pub(crate) fn loader(root: &Path, name: &str) -> Option<Value> {
    let mut name = name.to_owned();
    for _ in 0..8 {
        let (namespace, path) = name.split_once(':').unwrap_or(("minecraft", &name));
        // Vanilla's models never name one: no read for them.
        if namespace == "minecraft" {
            return None;
        }
        let file = root.join(format!("assets/{namespace}/models/{path}.json"));
        let text = fs::read_to_string(file).ok()?;
        let json: Value =
            serde_json::from_str(text.strip_prefix('\u{feff}').unwrap_or(&text)).ok()?;
        if let Some(loader) = json.get("loader").and_then(Value::as_str) {
            return loader.starts_with("neoforge:").then_some(json);
        }
        if json.get("elements").is_some() {
            return None;
        }
        name = json.get("parent")?.as_str()?.to_owned();
    }
    None
}

/// The quads of a model with a NeoForge loader: `json` is its file,
/// `resolved` the same model through the library (its textures and its
/// parents'), turned as its blockstate variant says.
pub(crate) fn bake(
    root: &Path,
    json: &Value,
    resolved: &ResolvedCuboidModel,
    models: &ParsedModels,
    transform: BlockModelTransform,
) -> Result<Vec<ModelQuad>> {
    match json.get("loader").and_then(Value::as_str) {
        Some("neoforge:obj") => obj(root, json, resolved, transform),
        Some("neoforge:composite") => {
            let mut out = Vec::new();
            let children = json.get("children").and_then(Value::as_object);
            for (name, child) in children.into_iter().flatten() {
                out.extend(composite_child(root, name, child, models, transform)?);
            }
            Ok(out)
        }
        other => Err(format!("model loader {other:?} is not one Horizon reads").into()),
    }
}

/// One of a composite's children: a model of its own, inline (with
/// `elements`, a `parent` or a loader of its own), or a model's id.
fn composite_child(
    root: &Path,
    name: &str,
    child: &Value,
    models: &ParsedModels,
    transform: BlockModelTransform,
) -> Result<Vec<ModelQuad>> {
    let (id, json) = match child {
        Value::String(id) => (id.clone(), loader(root, id)),
        Value::Object(_) => {
            let id = format!("horizon:composite_child/{name}");
            let loaded = child.get("loader").is_some().then(|| child.clone());
            (id, loaded)
        }
        _ => return Err(format!("composite child {name}: not a model").into()),
    };
    let model = match child {
        Value::Object(_) => Some(CuboidModel::from_json(&child.to_string())?),
        _ => None,
    };
    let library = models.library_with(root, &id, model)?;
    let resolved = library.resolve(&id)?;
    match json {
        Some(json) => bake(root, &json, &resolved, models, transform),
        None => Ok(bake_block_model(&resolved, transform)?),
    }
}

/// `neoforge:obj`: the OBJ file's faces as quads, a triangle as a quad with
/// its last corner twice, a larger face as a fan of those.
fn obj(
    root: &Path,
    json: &Value,
    resolved: &ResolvedCuboidModel,
    transform: BlockModelTransform,
) -> Result<Vec<ModelQuad>> {
    let file = json
        .get("model")
        .and_then(Value::as_str)
        .ok_or("an OBJ model names no `model`")?;
    let (namespace, path) = file.split_once(':').unwrap_or(("minecraft", file));
    let path = root.join(format!("assets/{namespace}/{path}"));
    let text = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let flip_v = json.get("flip_v").and_then(Value::as_bool).unwrap_or(false);
    let dir = path.parent().unwrap_or(root);
    let parsed = parse_obj(&text, |lib| fs::read_to_string(dir.join(lib)).ok())?;
    let rows = transform.matrix_rows();
    let turn = |p: [f32; 3]| {
        let v = [p[0] - 0.5, p[1] - 0.5, p[2] - 0.5];
        let r = |i: usize| rows[i][0] * v[0] + rows[i][1] * v[1] + rows[i][2] * v[2] + rows[i][3];
        [r(0) + 0.5, r(1) + 0.5, r(2) + 0.5]
    };
    let mut out = Vec::new();
    for face in &parsed.faces {
        let material = match parsed.textures.get(&face.material).map(String::as_str) {
            Some(slot) if slot.starts_with('#') => resolved.material(slot)?.cloned(),
            Some(texture) => Some(Material {
                sprite: texture.to_owned(),
                force_translucent: false,
            }),
            None => None,
        }
        .unwrap_or(Material {
            sprite: "minecraft:missingno".to_owned(),
            force_translucent: false,
        });
        let corner = |&(v, t): &(usize, Option<usize>)| {
            let position = turn(parsed.positions[v]);
            let uv = t.map_or([0.0, 0.0], |t| parsed.uvs[t]);
            (position, [uv[0], if flip_v { 1.0 - uv[1] } else { uv[1] }])
        };
        let corners: Vec<_> = face.corners.iter().map(corner).collect();
        for i in (1..corners.len().saturating_sub(1)).step_by(2) {
            let quad = [
                corners[0],
                corners[i],
                corners[i + 1],
                *corners.get(i + 2).unwrap_or(&corners[i + 1]),
            ];
            out.push(ModelQuad {
                positions: quad.map(|c| c.0),
                uv: quad.map(|c| c.1),
                facing: facing(quad.map(|c| c.0)),
                cull: None,
                material: material.clone(),
                tint_index: -1,
                shade_direction_override: None,
                light_emission: 0,
            });
        }
    }
    Ok(out)
}

/// The block face a quad looks most nearly towards, by its corners' normal.
fn facing(p: [[f32; 3]; 4]) -> Direction {
    let a = [p[1][0] - p[0][0], p[1][1] - p[0][1], p[1][2] - p[0][2]];
    let b = [p[2][0] - p[0][0], p[2][1] - p[0][1], p[2][2] - p[0][2]];
    let n = [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ];
    let axis = (0..3)
        .max_by(|&i, &j| n[i].abs().total_cmp(&n[j].abs()))
        .unwrap_or(1);
    match (axis, n[axis] < 0.0) {
        (0, true) => Direction::West,
        (0, false) => Direction::East,
        (1, true) => Direction::Down,
        (1, false) => Direction::Up,
        (2, true) => Direction::North,
        _ => Direction::South,
    }
}

/// An OBJ file's geometry: positions, texture coordinates, each face's
/// corners (position and coordinate indices) and material, and each
/// material's `map_Kd`.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct Obj {
    pub positions: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub faces: Vec<ObjFace>,
    pub textures: std::collections::BTreeMap<String, String>,
}

#[derive(Debug, PartialEq)]
pub(crate) struct ObjFace {
    pub corners: Vec<(usize, Option<usize>)>,
    pub material: String,
}

/// Reads an OBJ file, its `mtllib`s through `read_mtl`. Indices count from
/// 1, negative ones back from the end; normals are left out.
pub(crate) fn parse_obj(text: &str, read_mtl: impl Fn(&str) -> Option<String>) -> Result<Obj> {
    let mut obj = Obj::default();
    let mut material = String::new();
    let index = |word: &str, len: usize| -> Result<usize> {
        let i: i64 = word.parse().map_err(|_| format!("OBJ index {word:?}"))?;
        let i = if i < 0 { len as i64 + i } else { i - 1 };
        usize::try_from(i)
            .ok()
            .filter(|i| *i < len)
            .ok_or_else(|| format!("OBJ index {word} out of range").into())
    };
    for line in text.lines() {
        let mut words = line.split_whitespace();
        let numbers = |words: std::str::SplitWhitespace| -> Vec<f32> {
            words.filter_map(|w| w.parse().ok()).collect()
        };
        match words.next() {
            Some("v") => {
                let v = numbers(words);
                if v.len() < 3 {
                    return Err(format!("OBJ vertex {line:?}").into());
                }
                obj.positions.push([v[0], v[1], v[2]]);
            }
            Some("vt") => {
                let v = numbers(words);
                obj.uvs
                    .push([*v.first().unwrap_or(&0.0), *v.get(1).unwrap_or(&0.0)]);
            }
            Some("usemtl") => material = words.collect::<Vec<_>>().join(" "),
            Some("mtllib") => {
                for lib in words {
                    let Some(mtl) = read_mtl(lib) else { continue };
                    let mut current = String::new();
                    for line in mtl.lines() {
                        let mut w = line.split_whitespace();
                        match w.next() {
                            Some("newmtl") => current = w.collect::<Vec<_>>().join(" "),
                            Some("map_Kd") => {
                                let texture = w.collect::<Vec<_>>().join(" ");
                                obj.textures.insert(current.clone(), texture);
                            }
                            _ => {}
                        }
                    }
                }
            }
            Some("f") => {
                let mut corners = Vec::new();
                for corner in words {
                    let mut parts = corner.split('/');
                    let v = index(parts.next().unwrap_or(""), obj.positions.len())?;
                    let t = match parts.next() {
                        Some(t) if !t.is_empty() => Some(index(t, obj.uvs.len())?),
                        _ => None,
                    };
                    corners.push((v, t));
                }
                if corners.len() >= 3 {
                    obj.faces.push(ObjFace {
                        corners,
                        material: material.clone(),
                    });
                }
            }
            _ => {}
        }
    }
    Ok(obj)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MTL: &str = "newmtl m_0\nmap_Kd #0\nnewmtl m_1\nmap_Kd create:block/axis\n";
    const OBJ: &str = "mtllib a.mtl\no part\nv 0 0 0\nv 1 0 0\nv 1 1 0\nv 0 1 0\n\
        vt 0 0\nvt 1 0\nvt 1 1\nvt 0 1\nvn 0 0 1\nusemtl m_0\nf 1/1/1 2/2/1 3/3/1 4/4/1\n\
        usemtl m_1\nf -4/-4 -3/-3 -2/-2\nf 1 2 3 4 1\n";

    #[test]
    fn obj_faces_read_with_their_materials() {
        let obj = parse_obj(OBJ, |lib| (lib == "a.mtl").then(|| MTL.to_owned())).unwrap();
        assert_eq!(obj.positions.len(), 4);
        assert_eq!(obj.uvs[2], [1.0, 1.0]);
        assert_eq!(obj.faces.len(), 3);
        assert_eq!(obj.faces[0].material, "m_0");
        assert_eq!(obj.faces[0].corners[3], (3, Some(3)));
        // Negative indices count back from the end.
        assert_eq!(obj.faces[1].corners[0], (0, Some(0)));
        assert_eq!(obj.faces[1].material, "m_1");
        assert_eq!(obj.faces[2].corners[1], (1, None));
        assert_eq!(obj.textures["m_0"], "#0");
        assert_eq!(obj.textures["m_1"], "create:block/axis");
        assert!(parse_obj("v 0 0 0\nf 1 2 3\n", |_| None).is_err());
    }

    #[test]
    fn a_quad_faces_the_way_its_corners_wind() {
        let up = [
            [0.0, 1.0, 0.0],
            [0.0, 1.0, 1.0],
            [1.0, 1.0, 1.0],
            [1.0, 1.0, 0.0],
        ];
        assert_eq!(facing(up), Direction::Up);
        let south = [
            [0.0, 0.0, 1.0],
            [1.0, 0.0, 1.0],
            [1.0, 1.0, 1.0],
            [0.0, 1.0, 1.0],
        ];
        assert_eq!(facing(south), Direction::South);
    }
}
