use super::{
    ArchiveFileProvenance, BinaryReader, ByteSpan, DecodeError, DecodeErrorKind, DecodeResult,
    DecodeSubject, DecoderContext,
};
use osrs_core::coords::ModelPoint;
use osrs_core::definitions::DefinitionIdentity;
use osrs_core::ids::{ModelId, TextureId};
use osrs_core::model::{
    FacePriority, ModelEncoding, ModelFormatIdentity, SkeletalVertexData, SourceModel,
    SourceModelParts, TextureMappingParameters, TextureTriangle, TextureTriangleIndex, Triangle,
};

const FFFD_FOOTER_LEN: usize = 26;
const FFFE_FOOTER_LEN: usize = 23;

/// Decode one target-era ModelData payload into immutable canonical source data.
///
/// Build 241 / OpenRS2 2727 was exhaustively probed in M4 and contains only
/// `FF FD` and `FF FE` model families. Historical `FF FF` and legacy payloads
/// fail explicitly instead of being guessed from older revisions.
pub fn decode_model_data(
    bytes: &[u8],
    context: &DecoderContext,
    source: &ArchiveFileProvenance,
    model_id: ModelId,
) -> DecodeResult<SourceModel> {
    let reader = BinaryReader::new(bytes, context, source)
        .with_subject(DecodeSubject::Model(model_id.get()));
    if bytes.len() < 2 {
        return Err(reader.invalid_value(
            "model encoding",
            "model payload is shorter than the two-byte format discriminator",
            ByteSpan::new(0, bytes.len()),
            None,
        ));
    }

    match &bytes[bytes.len() - 2..] {
        [0xff, 0xfd] => decode_fffd(bytes, context, source, model_id),
        [0xff, 0xfe] => decode_fffe(bytes, context, source, model_id),
        [0xff, 0xff] => Err(reader.invalid_value(
            "model encoding",
            "FF FF is not present in the pinned build-241 target and is not implemented by M4 target decode",
            ByteSpan::new(bytes.len() - 2, 2),
            None,
        )),
        _ => Err(reader.invalid_value(
            "model encoding",
            "legacy ModelData is not present in the pinned build-241 target and is not implemented by M4 target decode",
            ByteSpan::new(bytes.len() - 2, 2),
            None,
        )),
    }
}

fn decode_fffd(
    bytes: &[u8],
    context: &DecoderContext,
    source: &ArchiveFileProvenance,
    model_id: ModelId,
) -> DecodeResult<SourceModel> {
    let root = BinaryReader::new(bytes, context, source)
        .with_subject(DecodeSubject::Model(model_id.get()));
    if bytes.len() < FFFD_FOOTER_LEN {
        return Err(root.invalid_value(
            "FF FD model length",
            format!("{} bytes is shorter than the {FFFD_FOOTER_LEN}-byte footer", bytes.len()),
            ByteSpan::new(0, bytes.len()),
            None,
        ));
    }
    let footer_offset = bytes.len() - FFFD_FOOTER_LEN;
    let mut footer = root.fork_at(footer_offset)?;
    let vertex_count = usize::from(footer.read_u16_be()?);
    let face_count = usize::from(footer.read_u16_be()?);
    let texture_count = usize::from(footer.read_u8()?);
    let has_face_render_types = read_flag(&mut footer, "face render types")?;
    let raw_priority = footer.read_u8()?;
    let has_face_alphas = read_flag(&mut footer, "face alphas")?;
    let has_face_skins = read_flag(&mut footer, "face skins")?;
    let has_face_textures = read_flag(&mut footer, "face textures")?;
    let has_vertex_skins = read_flag(&mut footer, "vertex skins")?;
    let has_skeletal = read_flag(&mut footer, "skeletal vertices")?;
    let x_len = usize::from(footer.read_u16_be()?);
    let y_len = usize::from(footer.read_u16_be()?);
    let z_len = usize::from(footer.read_u16_be()?);
    let face_index_len = usize::from(footer.read_u16_be()?);
    let texture_selector_len = usize::from(footer.read_u16_be()?);
    let vertex_meta_len = usize::from(footer.read_u16_be()?);
    expect_offset(&footer, bytes.len() - 2, "FF FD footer")?;

    let (default_priority, face_priorities) = priority_storage(raw_priority, face_count, &root)?;

    let mut texture_type_reader = root.fork_at(0)?;
    let mut texture_render_types = Vec::with_capacity(texture_count);
    let mut simple_texture_count = 0usize;
    let mut complex_texture_count = 0usize;
    let mut cube_texture_count = 0usize;
    for _ in 0..texture_count {
        let offset = texture_type_reader.offset();
        let render_type = texture_type_reader.read_i8()?;
        match render_type {
            0 => simple_texture_count += 1,
            1 | 3 => complex_texture_count += 1,
            2 => {
                complex_texture_count += 1;
                cube_texture_count += 1;
            }
            _ => {
                return Err(texture_type_reader.invalid_value(
                    "texture render type",
                    format!("unsupported FF FD texture render type {render_type}"),
                    ByteSpan::new(offset, 1),
                    None,
                ));
            }
        }
        texture_render_types.push(render_type);
    }

    let vertex_flags_offset = texture_count;
    let mut offset = checked_add(texture_count, vertex_count, &root, "vertex flags")?;
    let face_render_types_offset = offset;
    if has_face_render_types {
        offset = checked_add(offset, face_count, &root, "face render types")?;
    }
    let face_compress_types_offset = offset;
    offset = checked_add(offset, face_count, &root, "face compression types")?;
    let face_priorities_offset = offset;
    if raw_priority == 255 {
        offset = checked_add(offset, face_count, &root, "face priorities")?;
    }
    let face_skins_offset = offset;
    if has_face_skins {
        offset = checked_add(offset, face_count, &root, "face skins")?;
    }
    let vertex_meta_offset = offset;
    offset = checked_add(offset, vertex_meta_len, &root, "vertex metadata")?;
    let face_alphas_offset = offset;
    if has_face_alphas {
        offset = checked_add(offset, face_count, &root, "face alphas")?;
    }
    let face_indices_offset = offset;
    offset = checked_add(offset, face_index_len, &root, "face index deltas")?;
    let face_textures_offset = offset;
    if has_face_textures {
        offset = checked_add(
            offset,
            checked_mul(face_count, 2, &root, "face texture bytes")?,
            &root,
            "face textures",
        )?;
    }
    let texture_selectors_offset = offset;
    offset = checked_add(offset, texture_selector_len, &root, "texture selectors")?;
    let face_colors_offset = offset;
    offset = checked_add(
        offset,
        checked_mul(face_count, 2, &root, "face color bytes")?,
        &root,
        "face colors",
    )?;
    let x_offset = offset;
    offset = checked_add(offset, x_len, &root, "vertex x deltas")?;
    let y_offset = offset;
    offset = checked_add(offset, y_len, &root, "vertex y deltas")?;
    let z_offset = offset;
    offset = checked_add(offset, z_len, &root, "vertex z deltas")?;
    let simple_textures_offset = offset;
    offset = checked_add(
        offset,
        checked_mul(simple_texture_count, 6, &root, "simple texture bytes")?,
        &root,
        "simple texture triangles",
    )?;
    let complex_textures_offset = offset;
    offset = checked_add(
        offset,
        checked_mul(complex_texture_count, 6, &root, "complex texture vertex bytes")?,
        &root,
        "complex texture triangles",
    )?;
    let texture_scale_offset = offset;
    offset = checked_add(
        offset,
        checked_mul(complex_texture_count, 6, &root, "texture scale bytes")?,
        &root,
        "texture scales",
    )?;
    let texture_rotation_offset = offset;
    offset = checked_add(
        offset,
        checked_mul(complex_texture_count, 2, &root, "texture rotation bytes")?,
        &root,
        "texture rotations",
    )?;
    let texture_direction_offset = offset;
    offset = checked_add(offset, complex_texture_count, &root, "texture directions")?;
    let texture_speed_offset = offset;
    let speed_bytes = checked_add(
        checked_mul(complex_texture_count, 2, &root, "texture speed bytes")?,
        checked_mul(cube_texture_count, 2, &root, "cube texture translation bytes")?,
        &root,
        "texture speed/translation bytes",
    )?;
    offset = checked_add(offset, speed_bytes, &root, "texture speed/translation")?;
    let tail_offset = offset;
    if tail_offset > footer_offset {
        return Err(root.invalid_value(
            "FF FD section layout",
            format!("decoded sections end at {tail_offset}, after footer start {footer_offset}"),
            ByteSpan::new(footer_offset, FFFD_FOOTER_LEN),
            None,
        ));
    }

    let (vertices, vertex_skins, skeletal_vertices) = decode_vertices(
        &root,
        vertex_count,
        vertex_flags_offset,
        x_offset,
        y_offset,
        z_offset,
        vertex_meta_offset,
        vertex_meta_offset + vertex_meta_len,
        has_vertex_skins,
        has_skeletal,
    )?;

    let mut colors = root.fork_at(face_colors_offset)?;
    let mut render_types_reader = root.fork_at(face_render_types_offset)?;
    let mut priorities_reader = root.fork_at(face_priorities_offset)?;
    let mut alphas_reader = root.fork_at(face_alphas_offset)?;
    let mut skins_reader = root.fork_at(face_skins_offset)?;
    let mut textures_reader = root.fork_at(face_textures_offset)?;
    let mut selectors_reader = root.fork_at(texture_selectors_offset)?;

    let mut face_colors = Vec::with_capacity(face_count);
    let mut face_render_types = has_face_render_types.then(|| Vec::with_capacity(face_count));
    let mut decoded_priorities = face_priorities;
    let mut face_alphas = has_face_alphas.then(|| Vec::with_capacity(face_count));
    let mut face_skins = has_face_skins.then(|| Vec::with_capacity(face_count));
    let mut face_textures = has_face_textures.then(|| Vec::with_capacity(face_count));
    let mut texture_face_selectors = (has_face_textures && texture_count > 0)
        .then(|| Vec::with_capacity(face_count));

    for face in 0..face_count {
        face_colors.push(colors.read_u16_be()?);
        if let Some(values) = &mut face_render_types {
            values.push(render_types_reader.read_i8()?);
        }
        if let Some(values) = &mut decoded_priorities {
            values.push(read_priority(&mut priorities_reader, face)?);
        }
        if let Some(values) = &mut face_alphas {
            values.push(alphas_reader.read_i8()?);
        }
        if let Some(values) = &mut face_skins {
            values.push(i32::from(skins_reader.read_u8()?));
        }
        let texture = if let Some(values) = &mut face_textures {
            let encoded = textures_reader.read_u16_be()?;
            let value = (encoded != 0).then(|| TextureId::new(u32::from(encoded - 1)));
            values.push(value);
            value
        } else {
            None
        };
        if let Some(values) = &mut texture_face_selectors {
            let selector = if texture.is_some() {
                let encoded = selectors_reader.read_u8()?;
                (encoded != 0).then(|| TextureTriangleIndex::new(u32::from(encoded - 1)))
            } else {
                None
            };
            values.push(selector);
        }
    }

    let faces = decode_faces(
        &root,
        face_count,
        face_indices_offset,
        face_compress_types_offset,
    )?;
    let texture_triangles = decode_fffd_texture_triangles(
        &root,
        &texture_render_types,
        simple_textures_offset,
        complex_textures_offset,
        texture_scale_offset,
        texture_rotation_offset,
        texture_direction_offset,
        texture_speed_offset,
        tail_offset,
    )?;
    let face_biases = decode_fffd_tail(&root, tail_offset, footer_offset, face_count)?;

    admit_model(
        &root,
        context,
        model_id,
        ModelEncoding::TrailerFfFd,
        vertices,
        faces,
        face_colors,
        default_priority,
        face_render_types,
        decoded_priorities,
        face_alphas,
        face_textures,
        texture_face_selectors,
        face_biases,
        texture_triangles,
        vertex_skins,
        face_skins,
        skeletal_vertices,
    )
}

fn decode_fffe(
    bytes: &[u8],
    context: &DecoderContext,
    source: &ArchiveFileProvenance,
    model_id: ModelId,
) -> DecodeResult<SourceModel> {
    let root = BinaryReader::new(bytes, context, source)
        .with_subject(DecodeSubject::Model(model_id.get()));
    if bytes.len() < FFFE_FOOTER_LEN {
        return Err(root.invalid_value(
            "FF FE model length",
            format!("{} bytes is shorter than the {FFFE_FOOTER_LEN}-byte footer", bytes.len()),
            ByteSpan::new(0, bytes.len()),
            None,
        ));
    }
    let footer_offset = bytes.len() - FFFE_FOOTER_LEN;
    let mut footer = root.fork_at(footer_offset)?;
    let vertex_count = usize::from(footer.read_u16_be()?);
    let face_count = usize::from(footer.read_u16_be()?);
    let texture_count = usize::from(footer.read_u8()?);
    let has_packed_face_info = read_flag(&mut footer, "packed face info")?;
    let raw_priority = footer.read_u8()?;
    let has_face_alphas = read_flag(&mut footer, "face alphas")?;
    let has_face_skins = read_flag(&mut footer, "face skins")?;
    let has_vertex_skins = read_flag(&mut footer, "vertex skins")?;
    let has_skeletal = read_flag(&mut footer, "skeletal vertices")?;
    let x_len = usize::from(footer.read_u16_be()?);
    let y_len = usize::from(footer.read_u16_be()?);
    let z_len = usize::from(footer.read_u16_be()?);
    let face_index_len = usize::from(footer.read_u16_be()?);
    let vertex_meta_len = usize::from(footer.read_u16_be()?);
    expect_offset(&footer, bytes.len() - 2, "FF FE footer")?;

    let (default_priority, face_priorities) = priority_storage(raw_priority, face_count, &root)?;

    let vertex_flags_offset = 0usize;
    let mut offset = vertex_count;
    let face_compress_types_offset = offset;
    offset = checked_add(offset, face_count, &root, "face compression types")?;
    let face_priorities_offset = offset;
    if raw_priority == 255 {
        offset = checked_add(offset, face_count, &root, "face priorities")?;
    }
    let face_skins_offset = offset;
    if has_face_skins {
        offset = checked_add(offset, face_count, &root, "face skins")?;
    }
    let packed_face_info_offset = offset;
    if has_packed_face_info {
        offset = checked_add(offset, face_count, &root, "packed face info")?;
    }
    let vertex_meta_offset = offset;
    offset = checked_add(offset, vertex_meta_len, &root, "vertex metadata")?;
    let face_alphas_offset = offset;
    if has_face_alphas {
        offset = checked_add(offset, face_count, &root, "face alphas")?;
    }
    let face_indices_offset = offset;
    offset = checked_add(offset, face_index_len, &root, "face index deltas")?;
    let face_colors_offset = offset;
    offset = checked_add(
        offset,
        checked_mul(face_count, 2, &root, "face color bytes")?,
        &root,
        "face colors",
    )?;
    let texture_triangles_offset = offset;
    offset = checked_add(
        offset,
        checked_mul(texture_count, 6, &root, "texture triangle bytes")?,
        &root,
        "texture triangles",
    )?;
    let x_offset = offset;
    offset = checked_add(offset, x_len, &root, "vertex x deltas")?;
    let y_offset = offset;
    offset = checked_add(offset, y_len, &root, "vertex y deltas")?;
    let z_offset = offset;
    offset = checked_add(offset, z_len, &root, "vertex z deltas")?;
    let tail_offset = offset;
    if tail_offset > footer_offset {
        return Err(root.invalid_value(
            "FF FE section layout",
            format!("decoded sections end at {tail_offset}, after footer start {footer_offset}"),
            ByteSpan::new(footer_offset, FFFE_FOOTER_LEN),
            None,
        ));
    }

    let (vertices, vertex_skins, skeletal_vertices) = decode_vertices(
        &root,
        vertex_count,
        vertex_flags_offset,
        x_offset,
        y_offset,
        z_offset,
        vertex_meta_offset,
        vertex_meta_offset + vertex_meta_len,
        has_vertex_skins,
        has_skeletal,
    )?;

    let mut colors = root.fork_at(face_colors_offset)?;
    let mut packed = root.fork_at(packed_face_info_offset)?;
    let mut priorities_reader = root.fork_at(face_priorities_offset)?;
    let mut alphas_reader = root.fork_at(face_alphas_offset)?;
    let mut skins_reader = root.fork_at(face_skins_offset)?;

    let mut face_colors = Vec::with_capacity(face_count);
    let mut face_render_types = has_packed_face_info.then(|| Vec::with_capacity(face_count));
    let mut decoded_priorities = face_priorities;
    let mut face_alphas = has_face_alphas.then(|| Vec::with_capacity(face_count));
    let mut face_skins = has_face_skins.then(|| Vec::with_capacity(face_count));
    let mut face_textures = has_packed_face_info.then(|| Vec::with_capacity(face_count));
    let mut texture_face_selectors = has_packed_face_info.then(|| Vec::with_capacity(face_count));

    for face in 0..face_count {
        let raw_color = colors.read_u16_be()?;
        if has_packed_face_info {
            let info = packed.read_u8()?;
            face_render_types.as_mut().expect("allocated above").push(if info & 1 == 1 { 1 } else { 0 });
            if info & 2 == 2 {
                let texture = (raw_color != u16::MAX).then(|| TextureId::new(u32::from(raw_color)));
                face_textures.as_mut().expect("allocated above").push(texture);
                texture_face_selectors
                    .as_mut()
                    .expect("allocated above")
                    .push(texture.map(|_| TextureTriangleIndex::new(u32::from(info >> 2))));
                face_colors.push(127);
            } else {
                face_textures.as_mut().expect("allocated above").push(None);
                texture_face_selectors.as_mut().expect("allocated above").push(None);
                face_colors.push(raw_color);
            }
        } else {
            face_colors.push(raw_color);
        }
        if let Some(values) = &mut decoded_priorities {
            values.push(read_priority(&mut priorities_reader, face)?);
        }
        if let Some(values) = &mut face_alphas {
            values.push(alphas_reader.read_i8()?);
        }
        if let Some(values) = &mut face_skins {
            values.push(i32::from(skins_reader.read_u8()?));
        }
    }

    let faces = decode_faces(
        &root,
        face_count,
        face_indices_offset,
        face_compress_types_offset,
    )?;
    let mut texture_reader = root.fork_at(texture_triangles_offset)?;
    let mut texture_triangles = Vec::with_capacity(texture_count);
    for _ in 0..texture_count {
        texture_triangles.push(TextureTriangle {
            render_type: 0,
            vertices: Triangle::new(
                u32::from(texture_reader.read_u16_be()?),
                u32::from(texture_reader.read_u16_be()?),
                u32::from(texture_reader.read_u16_be()?),
            ),
            mapping: TextureMappingParameters::default(),
        });
    }
    expect_offset(&texture_reader, x_offset, "FF FE texture triangles")?;
    let face_biases = decode_bias_tail(&root, tail_offset, footer_offset, face_count, "FF FE")?;

    admit_model(
        &root,
        context,
        model_id,
        ModelEncoding::TrailerFfFe,
        vertices,
        faces,
        face_colors,
        default_priority,
        face_render_types,
        decoded_priorities,
        face_alphas,
        face_textures,
        texture_face_selectors,
        face_biases,
        texture_triangles,
        vertex_skins,
        face_skins,
        skeletal_vertices,
    )
}

#[allow(clippy::too_many_arguments)]
fn decode_vertices(
    root: &BinaryReader<'_>,
    vertex_count: usize,
    flags_offset: usize,
    x_offset: usize,
    y_offset: usize,
    z_offset: usize,
    metadata_offset: usize,
    metadata_end: usize,
    has_vertex_skins: bool,
    has_skeletal: bool,
) -> DecodeResult<(
    Vec<ModelPoint>,
    Option<Vec<i32>>,
    Option<Vec<Option<SkeletalVertexData>>>,
)> {
    let mut flags = root.fork_at(flags_offset)?;
    let mut xs = root.fork_at(x_offset)?;
    let mut ys = root.fork_at(y_offset)?;
    let mut zs = root.fork_at(z_offset)?;
    let mut metadata = root.fork_at(metadata_offset)?;
    let mut vertices = Vec::with_capacity(vertex_count);
    let mut vertex_skins = has_vertex_skins.then(|| Vec::with_capacity(vertex_count));
    let mut x = 0i32;
    let mut y = 0i32;
    let mut z = 0i32;

    for _ in 0..vertex_count {
        let flag = flags.read_u8()?;
        let dx = if flag & 1 != 0 { xs.read_short_smart()? } else { 0 };
        let dy = if flag & 2 != 0 { ys.read_short_smart()? } else { 0 };
        let dz = if flag & 4 != 0 { zs.read_short_smart()? } else { 0 };
        x = x.wrapping_add(dx);
        y = y.wrapping_add(dy);
        z = z.wrapping_add(dz);
        vertices.push(ModelPoint::new(x, y, z));
        if let Some(values) = &mut vertex_skins {
            values.push(i32::from(metadata.read_u8()?));
        }
    }

    let skeletal_vertices = if has_skeletal {
        let mut values = Vec::with_capacity(vertex_count);
        for _ in 0..vertex_count {
            let influence_count = usize::from(metadata.read_u8()?);
            let mut bone_ids = Vec::with_capacity(influence_count);
            let mut weights = Vec::with_capacity(influence_count);
            for _ in 0..influence_count {
                bone_ids.push(i32::from(metadata.read_u8()?));
                weights.push(i32::from(metadata.read_u8()?));
            }
            values.push(Some(SkeletalVertexData { bone_ids, weights }));
        }
        Some(values)
    } else {
        None
    };
    expect_offset(&metadata, metadata_end, "vertex metadata")?;
    Ok((vertices, vertex_skins, skeletal_vertices))
}

fn decode_faces(
    root: &BinaryReader<'_>,
    face_count: usize,
    delta_offset: usize,
    compression_offset: usize,
) -> DecodeResult<Vec<Triangle>> {
    let mut deltas = root.fork_at(delta_offset)?;
    let mut compression = root.fork_at(compression_offset)?;
    let mut faces = Vec::with_capacity(face_count);
    let mut a = 0i32;
    let mut b = 0i32;
    let mut c = 0i32;
    let mut last = 0i32;

    for face in 0..face_count {
        let mode_offset = compression.offset();
        let mode = compression.read_u8()?;
        match mode {
            1 => {
                a = deltas.read_short_smart()?.wrapping_add(last);
                b = deltas.read_short_smart()?.wrapping_add(a);
                c = deltas.read_short_smart()?.wrapping_add(b);
                last = c;
            }
            2 => {
                b = c;
                c = deltas.read_short_smart()?.wrapping_add(last);
                last = c;
            }
            3 => {
                a = c;
                c = deltas.read_short_smart()?.wrapping_add(last);
                last = c;
            }
            4 => {
                std::mem::swap(&mut a, &mut b);
                c = deltas.read_short_smart()?.wrapping_add(last);
                last = c;
            }
            _ => {
                return Err(compression.invalid_value(
                    "face compression type",
                    format!("face {face} uses unsupported compression type {mode}"),
                    ByteSpan::new(mode_offset, 1),
                    None,
                ));
            }
        }
        let au = face_index(a, face, "a", root)?;
        let bu = face_index(b, face, "b", root)?;
        let cu = face_index(c, face, "c", root)?;
        faces.push(Triangle::new(au, bu, cu));
    }
    Ok(faces)
}

#[allow(clippy::too_many_arguments)]
fn decode_fffd_texture_triangles(
    root: &BinaryReader<'_>,
    render_types: &[i8],
    simple_offset: usize,
    complex_offset: usize,
    scale_offset: usize,
    rotation_offset: usize,
    direction_offset: usize,
    speed_offset: usize,
    data_end: usize,
) -> DecodeResult<Vec<TextureTriangle>> {
    let mut simple = root.fork_at(simple_offset)?;
    let mut complex = root.fork_at(complex_offset)?;
    let mut scales = root.fork_at(scale_offset)?;
    let mut rotations = root.fork_at(rotation_offset)?;
    let mut directions = root.fork_at(direction_offset)?;
    let mut speeds = root.fork_at(speed_offset)?;
    let mut triangles = Vec::with_capacity(render_types.len());

    for &render_type in render_types {
        let (vertices, mapping) = if render_type == 0 {
            (
                Triangle::new(
                    u32::from(simple.read_u16_be()?),
                    u32::from(simple.read_u16_be()?),
                    u32::from(simple.read_u16_be()?),
                ),
                TextureMappingParameters::default(),
            )
        } else {
            let vertices = Triangle::new(
                u32::from(complex.read_u16_be()?),
                u32::from(complex.read_u16_be()?),
                u32::from(complex.read_u16_be()?),
            );
            let scale = [
                i32::from(scales.read_u16_be()?),
                i32::from(scales.read_u16_be()?),
                i32::from(scales.read_u16_be()?),
            ];
            let rotation = i32::from(rotations.read_u16_be()?);
            let direction = i32::from(directions.read_i8()?);
            let speed = i32::from(speeds.read_u16_be()?);
            let translation = if render_type == 2 {
                Some([i32::from(speeds.read_i8()?), i32::from(speeds.read_i8()?)])
            } else {
                None
            };
            (
                vertices,
                TextureMappingParameters {
                    scale: Some(scale),
                    rotation: Some(rotation),
                    direction: Some(direction),
                    speed: Some(speed),
                    translation,
                },
            )
        };
        triangles.push(TextureTriangle {
            render_type,
            vertices,
            mapping,
        });
    }
    expect_offset(&simple, complex_offset, "simple texture stream")?;
    expect_offset(&complex, scale_offset, "complex texture vertex stream")?;
    expect_offset(&scales, rotation_offset, "texture scale stream")?;
    expect_offset(&rotations, direction_offset, "texture rotation stream")?;
    expect_offset(&directions, speed_offset, "texture direction stream")?;
    expect_offset(&speeds, data_end, "texture speed/translation stream")?;
    Ok(triangles)
}

fn decode_fffd_tail(
    root: &BinaryReader<'_>,
    tail_offset: usize,
    footer_offset: usize,
    face_count: usize,
) -> DecodeResult<Option<Vec<i8>>> {
    let mut tail = root.fork_at(tail_offset)?;
    if tail.offset() >= footer_offset {
        return Err(root.invalid_value(
            "FF FD trailer metadata",
            "missing extension marker and face-bias presence flag",
            ByteSpan::new(tail_offset, footer_offset.saturating_sub(tail_offset)),
            None,
        ));
    }
    let marker = tail.read_u8()?;
    if marker != 0 {
        tail.skip(10)?;
    }
    decode_bias_tail_from_reader(root, &mut tail, footer_offset, face_count, "FF FD")
}

fn decode_bias_tail(
    root: &BinaryReader<'_>,
    tail_offset: usize,
    footer_offset: usize,
    face_count: usize,
    family: &'static str,
) -> DecodeResult<Option<Vec<i8>>> {
    let mut tail = root.fork_at(tail_offset)?;
    decode_bias_tail_from_reader(root, &mut tail, footer_offset, face_count, family)
}

fn decode_bias_tail_from_reader(
    root: &BinaryReader<'_>,
    tail: &mut BinaryReader<'_>,
    footer_offset: usize,
    face_count: usize,
    family: &'static str,
) -> DecodeResult<Option<Vec<i8>>> {
    if tail.offset() >= footer_offset {
        return Err(root.invalid_value(
            "face-bias presence flag",
            format!("{family} model has no face-bias presence byte before its footer"),
            ByteSpan::new(tail.offset(), 0),
            None,
        ));
    }
    let flag_offset = tail.offset();
    let has_bias = match tail.read_u8()? {
        0 => false,
        1 => true,
        value => {
            return Err(root.invalid_value(
                "face-bias presence flag",
                format!("{family} model uses invalid flag {value}"),
                ByteSpan::new(flag_offset, 1),
                None,
            ));
        }
    };
    let biases = if has_bias {
        let mut values = Vec::with_capacity(face_count);
        for _ in 0..face_count {
            values.push(tail.read_i8()?);
        }
        Some(values)
    } else {
        None
    };
    expect_offset(tail, footer_offset, "model trailer metadata")?;
    Ok(biases)
}

#[allow(clippy::too_many_arguments)]
fn admit_model(
    root: &BinaryReader<'_>,
    context: &DecoderContext,
    model_id: ModelId,
    encoding: ModelEncoding,
    vertices: Vec<ModelPoint>,
    faces: Vec<Triangle>,
    face_colors: Vec<u16>,
    default_priority: FacePriority,
    face_render_types: Option<Vec<i8>>,
    face_priorities: Option<Vec<FacePriority>>,
    face_alphas: Option<Vec<i8>>,
    face_textures: Option<Vec<Option<TextureId>>>,
    texture_face_selectors: Option<Vec<Option<TextureTriangleIndex>>>,
    face_biases: Option<Vec<i8>>,
    texture_triangles: Vec<TextureTriangle>,
    vertex_skins: Option<Vec<i32>>,
    face_skins: Option<Vec<i32>>,
    skeletal_vertices: Option<Vec<Option<SkeletalVertexData>>>,
) -> DecodeResult<SourceModel> {
    SourceModel::from_parts(SourceModelParts {
        identity: DefinitionIdentity::new(model_id, context.target_provenance().clone()),
        format: ModelFormatIdentity {
            encoding,
            version: None,
        },
        vertices,
        faces,
        face_colors,
        default_priority,
        face_render_types,
        face_priorities,
        face_alphas,
        face_textures,
        texture_face_selectors,
        face_biases,
        texture_triangles,
        vertex_skins,
        face_skins,
        skeletal_vertices,
    })
    .map_err(|error| {
        root.invalid_value(
            "canonical model",
            error.to_string(),
            ByteSpan::new(0, root.len()),
            None,
        )
    })
}

fn read_flag(reader: &mut BinaryReader<'_>, field: &'static str) -> DecodeResult<bool> {
    let offset = reader.offset();
    match reader.read_u8()? {
        0 => Ok(false),
        1 => Ok(true),
        value => Err(reader.invalid_value(
            field,
            format!("expected 0 or 1, found {value}"),
            ByteSpan::new(offset, 1),
            None,
        )),
    }
}

fn priority_storage(
    raw: u8,
    face_count: usize,
    root: &BinaryReader<'_>,
) -> DecodeResult<(FacePriority, Option<Vec<FacePriority>>)> {
    if raw == 255 {
        return Ok((FacePriority::ZERO, Some(Vec::with_capacity(face_count))));
    }
    let Some(priority) = FacePriority::new(raw) else {
        return Err(root.invalid_value(
            "default face priority",
            format!("priority {raw} is outside 0..=11"),
            ByteSpan::new(0, root.len()),
            None,
        ));
    };
    Ok((priority, None))
}

fn read_priority(reader: &mut BinaryReader<'_>, face: usize) -> DecodeResult<FacePriority> {
    let offset = reader.offset();
    let raw = reader.read_u8()?;
    FacePriority::new(raw).ok_or_else(|| {
        reader.invalid_value(
            "face priority",
            format!("face {face} priority {raw} is outside 0..=11"),
            ByteSpan::new(offset, 1),
            None,
        )
    })
}

fn face_index(
    value: i32,
    face: usize,
    component: &'static str,
    root: &BinaryReader<'_>,
) -> DecodeResult<u32> {
    u32::try_from(value).map_err(|_| {
        root.invalid_value(
            "face vertex index",
            format!("face {face} component {component} decoded negative index {value}"),
            ByteSpan::new(0, root.len()),
            None,
        )
    })
}

fn checked_mul(
    left: usize,
    right: usize,
    root: &BinaryReader<'_>,
    field: &'static str,
) -> DecodeResult<usize> {
    left.checked_mul(right).ok_or_else(|| {
        root.invalid_value(
            field,
            "section size overflow",
            ByteSpan::new(0, root.len()),
            None,
        )
    })
}

fn checked_add(
    left: usize,
    right: usize,
    root: &BinaryReader<'_>,
    field: &'static str,
) -> DecodeResult<usize> {
    left.checked_add(right).ok_or_else(|| {
        root.invalid_value(
            field,
            "section offset overflow",
            ByteSpan::new(0, root.len()),
            None,
        )
    })
}

fn expect_offset(
    reader: &BinaryReader<'_>,
    expected: usize,
    field: &'static str,
) -> DecodeResult<()> {
    if reader.offset() == expected {
        return Ok(());
    }
    Err(reader.invalid_value(
        field,
        format!("cursor ended at {}, expected {expected}", reader.offset()),
        ByteSpan::new(reader.offset().min(reader.len()), 0),
        None,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decode::test_support;

    fn model_source() -> ArchiveFileProvenance {
        ArchiveFileProvenance::new(7, 53_512, Some(0))
    }

    #[test]
    fn decodes_pinned_target_fffd_sample_exactly() -> Result<(), Box<dyn std::error::Error>> {
        let context = test_support::target_context()?;
        let bytes = [
            0, 5, 1, 1, 64, 65, 65, 0, 0, 63, 65, 65, 0, 0, 0, 3, 0, 1, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 2, 0, 0, 0, 1, 0, 3, 0, 0, 0, 0, 255, 253,
        ];
        let model = decode_model_data(&bytes, &context, &model_source(), ModelId::new(53_512))?;

        assert_eq!(model.format().encoding, ModelEncoding::TrailerFfFd);
        assert_eq!(
            model.vertices(),
            &[
                ModelPoint::new(0, 0, 0),
                ModelPoint::new(-1, 0, 1),
                ModelPoint::new(0, 0, 1),
            ]
        );
        assert_eq!(model.faces(), &[Triangle::new(0, 1, 2)]);
        assert_eq!(model.face_colors(), &[0]);
        assert!(model.face_render_types().is_none());
        assert!(model.face_textures().is_none());
        assert!(model.face_biases().is_none());
        Ok(())
    }

    #[test]
    fn decodes_minimal_fffe_payload() -> Result<(), Box<dyn std::error::Error>> {
        let context = test_support::target_context()?;
        let mut bytes = vec![0u8];
        bytes.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        bytes.extend_from_slice(&[255, 254]);
        let model = decode_model_data(&bytes, &context, &model_source(), ModelId::new(596))?;
        assert_eq!(model.format().encoding, ModelEncoding::TrailerFfFe);
        assert!(model.vertices().is_empty());
        assert!(model.faces().is_empty());
        Ok(())
    }

    #[test]
    fn unsupported_target_absent_formats_fail_explicitly() -> Result<(), Box<dyn std::error::Error>> {
        let context = test_support::target_context()?;
        for bytes in [&[0u8, 0xff, 0xff][..], &[0u8, 0, 0][..]] {
            let error = decode_model_data(bytes, &context, &model_source(), ModelId::new(9))
                .expect_err("unsupported model family unexpectedly decoded");
            assert_eq!(error.subject(), Some(&DecodeSubject::Model(9)));
            assert!(matches!(error.kind(), DecodeErrorKind::InvalidValue { field: "model encoding", .. }));
        }
        Ok(())
    }

    #[test]
    fn truncated_target_formats_fail_with_model_provenance() -> Result<(), Box<dyn std::error::Error>> {
        let context = test_support::target_context()?;
        for bytes in [&[0xff, 0xfd][..], &[0xff, 0xfe][..]] {
            let error = decode_model_data(bytes, &context, &model_source(), ModelId::new(42))
                .expect_err("truncated model unexpectedly decoded");
            assert_eq!(error.subject(), Some(&DecodeSubject::Model(42)));
            assert!(matches!(error.kind(), DecodeErrorKind::InvalidValue { .. }));
        }
        Ok(())
    }
}
