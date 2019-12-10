fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::compile_protos("src/controller/proto/rrset.proto")?;
    tonic_build::compile_protos("src/controller/proto/dynamic_update_interface.proto")?;
    Ok(())
}
