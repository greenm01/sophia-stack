use super::*;

pub(crate) fn encode_surface_id(id: SurfaceId, out: &mut Vec<u8>) {
    push_u32(out, id.index());
    push_u32(out, id.generation());
}

pub(crate) fn decode_surface_id(cursor: &mut Cursor<'_>) -> Result<SurfaceId, IpcCodecError> {
    Ok(SurfaceId::new(cursor.u32()?, cursor.u32()?))
}
