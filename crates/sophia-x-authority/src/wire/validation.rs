fn require_len(opcode: u8, expected_at_least: usize, actual: usize) -> Result<(), XWireParseError> {
    if actual < expected_at_least {
        return Err(XWireParseError::InvalidLength {
            opcode,
            expected_at_least,
            actual,
        });
    }
    Ok(())
}

fn require_exact_len(opcode: u8, expected: usize, actual: usize) -> Result<(), XWireParseError> {
    if actual != expected {
        return Err(XWireParseError::InvalidLength {
            opcode,
            expected_at_least: expected,
            actual,
        });
    }
    Ok(())
}

fn validate_wire_property_format(format: u8) -> Result<(), XWireParseError> {
    match format {
        8 | 16 | 32 => Ok(()),
        other => Err(XWireParseError::InvalidPropertyFormat(other)),
    }
}

fn validate_wire_image_format(format: u8) -> Result<(), XWireParseError> {
    match format {
        0..=2 => Ok(()),
        other => Err(XWireParseError::InvalidPropertyFormat(other)),
    }
}
