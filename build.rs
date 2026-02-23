use std::io::Result;
fn main() -> Result<()> {
    tonic_prost_build::configure().compile_protos(&["task.proto"], &["./"])?;
    Ok(())
}
