from pathlib import Path


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected one match, found {count}")
    return text.replace(old, new, 1)


error_path = Path("crates/osrs-cache/src/decode/error.rs")
error = error_path.read_text()
error = replace_once(
    error,
    """    Texture(u32),\n    Sequence(u32),\n    TerrainRegion { x: i32, y: i32 },\n""",
    """    Texture(u32),\n    Sequence(u32),\n    Model(u32),\n    TerrainRegion { x: i32, y: i32 },\n""",
    "model decode subject enum",
)
error = replace_once(
    error,
    """            Self::Texture(id) => write!(formatter, \"texture:{id}\"),\n            Self::Sequence(id) => write!(formatter, \"sequence:{id}\"),\n            Self::TerrainRegion { x, y } => write!(formatter, \"terrain-region:{x},{y}\"),\n""",
    """            Self::Texture(id) => write!(formatter, \"texture:{id}\"),\n            Self::Sequence(id) => write!(formatter, \"sequence:{id}\"),\n            Self::Model(id) => write!(formatter, \"model:{id}\"),\n            Self::TerrainRegion { x, y } => write!(formatter, \"terrain-region:{x},{y}\"),\n""",
    "model decode subject display",
)
error = replace_once(
    error,
    """    #[test]\n    fn xtea_provenance_rejects_missing_identity() {\n""",
    """    #[test]\n    fn model_subject_renders_stable_identity() {\n        assert_eq!(DecodeSubject::Model(u32::MAX).to_string(), \"model:4294967295\");\n    }\n\n    #[test]\n    fn xtea_provenance_rejects_missing_identity() {\n""",
    "model subject test",
)
error_path.write_text(error)

reader_path = Path("crates/osrs-cache/src/decode/reader.rs")
reader = reader_path.read_text()
reader = replace_once(
    reader,
    """    pub fn with_subject(mut self, subject: DecodeSubject) -> Self {\n        self.subject = Some(subject);\n        self\n    }\n\n    pub const fn offset(&self) -> usize {\n""",
    """    pub fn with_subject(mut self, subject: DecodeSubject) -> Self {\n        self.subject = Some(subject);\n        self\n    }\n\n    /// Move this reader to an absolute byte offset without losing provenance.\n    ///\n    /// ModelData uses several independent cursors over one byte slice. Keeping\n    /// cursor movement inside this checked reader prevents decoder-local slicing\n    /// arithmetic from becoming a panic surface.\n    pub fn seek(&mut self, offset: usize) -> DecodeResult<()> {\n        if offset <= self.bytes.len() {\n            self.offset = offset;\n            return Ok(());\n        }\n        Err(self.error_at(\n            self.bytes.len(),\n            0,\n            None,\n            DecodeErrorKind::InvalidValue {\n                field: \"reader offset\",\n                detail: format!(\n                    \"offset {offset} exceeds input length {}\",\n                    self.bytes.len()\n                ),\n            },\n        ))\n    }\n\n    /// Create an independent checked cursor over the same bytes and provenance.\n    pub fn fork_at(&self, offset: usize) -> DecodeResult<Self> {\n        let mut fork = Self {\n            bytes: self.bytes,\n            offset: self.offset,\n            context: self.context,\n            source: self.source,\n            subject: self.subject.clone(),\n        };\n        fork.seek(offset)?;\n        Ok(fork)\n    }\n\n    pub const fn offset(&self) -> usize {\n""",
    "reader absolute cursor support",
)
reader = replace_once(
    reader,
    """    pub fn read_unsigned_short_smart(&mut self) -> DecodeResult<u16> {\n        if self.peek_u8()? < 128 {\n            self.read_u8().map(u16::from)\n        } else {\n            self.read_u16_be().map(|value| value - 32_768)\n        }\n    }\n\n    /// Read a NUL-terminated byte field without allocating.\n""",
    """    pub fn read_unsigned_short_smart(&mut self) -> DecodeResult<u16> {\n        if self.peek_u8()? < 128 {\n            self.read_u8().map(u16::from)\n        } else {\n            self.read_u16_be().map(|value| value - 32_768)\n        }\n    }\n\n    /// RuneScape signed short-smart primitive used by ModelData delta streams.\n    ///\n    /// This matches the pinned client's `Buffer.readShortSmart()` exactly:\n    /// one-byte values subtract 64 and two-byte values subtract 49152.\n    pub fn read_short_smart(&mut self) -> DecodeResult<i32> {\n        if self.peek_u8()? < 128 {\n            self.read_u8().map(|value| i32::from(value) - 64)\n        } else {\n            self.read_u16_be().map(|value| i32::from(value) - 49_152)\n        }\n    }\n\n    /// Read a NUL-terminated byte field without allocating.\n""",
    "signed short smart",
)
reader = replace_once(
    reader,
    """    #[test]\n    fn truncated_reads_return_contextual_error_without_advancing()\n""",
    """    #[test]\n    fn signed_short_smart_matches_modeldata_delta_domain()\n    -> Result<(), Box<dyn std::error::Error>> {\n        let context = test_support::target_context()?;\n        let source = test_support::source();\n        let bytes = [0, 63, 64, 127, 0x80, 0x00, 0xbf, 0xff, 0xff, 0xff];\n        let mut reader = BinaryReader::new(&bytes, &context, &source);\n\n        assert_eq!(reader.read_short_smart()?, -64);\n        assert_eq!(reader.read_short_smart()?, -1);\n        assert_eq!(reader.read_short_smart()?, 0);\n        assert_eq!(reader.read_short_smart()?, 63);\n        assert_eq!(reader.read_short_smart()?, -16_384);\n        assert_eq!(reader.read_short_smart()?, -1);\n        assert_eq!(reader.read_short_smart()?, 16_383);\n        reader.finish()?;\n        Ok(())\n    }\n\n    #[test]\n    fn forked_model_cursor_is_independent_bounded_and_preserves_subject()\n    -> Result<(), Box<dyn std::error::Error>> {\n        let context = test_support::target_context()?;\n        let source = ArchiveFileProvenance::new(7, 123, Some(0));\n        let bytes = [10, 20, 30, 40];\n        let mut reader = BinaryReader::new(&bytes, &context, &source)\n            .with_subject(DecodeSubject::Model(123));\n        let mut fork = reader.fork_at(2)?;\n\n        assert_eq!(fork.read_u8()?, 30);\n        assert_eq!(reader.offset(), 0);\n        reader.seek(4)?;\n        assert_eq!(reader.remaining(), 0);\n        let error = match reader.seek(5) {\n            Err(error) => error,\n            Ok(()) => return Err(\"out-of-range model cursor unexpectedly succeeded\".into()),\n        };\n        assert_eq!(error.subject(), Some(&DecodeSubject::Model(123)));\n        assert_eq!(error.span(), ByteSpan::new(4, 0));\n        assert!(matches!(\n            error.kind(),\n            DecodeErrorKind::InvalidValue { field: \"reader offset\", .. }\n        ));\n        Ok(())\n    }\n\n    #[test]\n    fn truncated_reads_return_contextual_error_without_advancing()\n""",
    "reader M4 tests",
)
reader_path.write_text(reader)

progress = """# M4 Progress: Model Decode and Exact Construction\n\nStatus: **Checkpoint 1 in progress**  \nBranch: `impl/m4-model-decode-construction`  \nBaseline: M3 squash merge `6e350afeb73c96f1b1ccd050ef427028e3015987`\n\nM4 owns raw ModelData decoding and the exact pre-GPU construction semantics required by `MODEL-BUILD-001..003`, `COORD-002`, and the M4 portions of `FACE-001`. It also establishes the ownership/cache foundation later consumed by `MODEL-BUILD-004/005`, M7 normals/lighting, and M8 contour/animation work.\n\n## Non-negotiable boundaries\n\n- No wgpu mesh is an M4 semantic authority or acceptance artifact.\n- No renderer-side negative scaling may substitute for semantic mirroring/winding.\n- No floating-point transform matrix may replace audited integer ModelData transform order.\n- Raw decoded source models remain immutable shared inputs after cache admission.\n- Scene normal reconciliation, final lighting, contouring, and animation execution remain with their owning later milestones.\n- Missing typed/untyped model selection is preserved as semantic absence, never replaced with fallback geometry.\n\n## Pinned decode evidence\n\nPrimary semantic pin: `melxin/runelite@1ad572d7dcdbc0fb67a4a00f0c2f959d5ab25abc`.\n\nThe pinned `ModelData(byte[])` constructor dispatches by the final two source bytes:\n\n| Trailer | Pinned decoder | Canonical `ModelEncoding` |\n|---|---|---|\n| `FF FD` | `method5265` | `TrailerFfFd` |\n| `FF FE` | `method5266` | `TrailerFfFe` |\n| `FF FF` | `method5287` | `TrailerFfFf` |\n| anything else | `method5268` | `Legacy` |\n\nThe same pin defines `Buffer.readShortSmart()` as:\n\n- first byte `< 128`: unsigned byte minus `64`;\n- otherwise: unsigned short minus `49152`.\n\nThat primitive drives ModelData vertex-coordinate deltas and face-index delta streams, so M4 uses one shared bounds-checked implementation rather than format-local copies.\n\n## Checkpoint sequence\n\n### Checkpoint 1: decoder substrate\n\n- [x] branch from exact M3 merge baseline;\n- [x] audit roadmap/spec/acceptance/parity contracts;\n- [x] add model ID to the standard decode provenance subject;\n- [x] add checked absolute/forked cursors for parallel ModelData streams;\n- [x] add exact signed short-smart primitive and boundary tests;\n- [ ] final branch CI readback.\n\n### Checkpoint 2: raw ModelData format decode\n\n- implement trailer-family dispatch;\n- probe/pin the format families actually encountered by target cache 2727;\n- implement all encountered build-241 format branches without destructive metadata filtering;\n- preserve vertices, topology, face metadata, texture triangles/mapping, skins, skeletal inputs, format identity, and provenance;\n- add malformed/truncated format tests.\n\n### Checkpoint 3: model source repository and reusable raw variants\n\n- read model index `7` through `CacheRepository`;\n- add profile/revision-aware raw model cache identity;\n- establish distinct mirrored raw variant keys;\n- prove immutable decoded source ownership.\n\n### Checkpoint 4: selection, combination, and mirror semantics\n\n- close `MODEL-BUILD-001`;\n- close `MODEL-BUILD-002`;\n- exact typed/untyped selection;\n- exact multi-model combine;\n- mirror vertices plus winding;\n- untyped/type-10 special-case behavior from pinned source.\n\n### Checkpoint 5: exact instance transform pipeline\n\n- type-4 `256` JAU recenter and `(45, 0, -45)` translation;\n- ordinary orientation quarter turns;\n- recolor;\n- retexture;\n- resize;\n- final definition translation;\n- combined order-sensitive exact integer fixture;\n- two-instance source immutability control.\n\n### Checkpoint 6: M4 verification closure\n\n- model decode fuzz smoke;\n- exact P0/P1 fixtures for `MODEL-BUILD-001..003` and `COORD-002`;\n- advance parity matrix rows to `EXISTING` only with linked artifacts;\n- M4 exit audit;\n- Tier A-C clean head before PR/merge.\n\n## Current checkpoint result\n\nCheckpoint 1 deliberately does not claim raw model decoding yet. It closes the shared cursor/provenance primitives that every format decoder needs and records the exact pinned dispatch contract before format-specific parsing begins.\n"""
Path("docs/implementation/M4-PROGRESS.md").write_text(progress)
