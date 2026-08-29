//! Evidence verification, reading records with the vocabulary that wrote them.

pub fn run(arguments: &[String]) -> Result<(), String> {
    match arguments.first().map(String::as_str) {
        Some("direct-scanout") => Err("not yet implemented".to_owned()),
        Some(other) => Err(format!("unknown verification {other:?}")),
        None => Err("verify needs a subject".to_owned()),
    }
}
