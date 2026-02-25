use std::io::Result;
fn main() -> Result<()> {
    // Box the single-Val fields that would otherwise create recursive size cycles:
    // Val → {Option,Result,Record,Variant}Val → Val
    tonic_prost_build::configure()
        .boxed(".task.OptionVal.value")
        .boxed(".task.ResultVal.value")
        .boxed(".task.RecordField.value")
        .boxed(".task.VariantVal.value")
        .compile_protos(&["task.proto"], &["./"])?;
    Ok(())
}
