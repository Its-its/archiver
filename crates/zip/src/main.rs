use tracing::debug;
use zip_archiver::{Archive, Result, CompressionType};

#[tokio::main]
async fn main() -> Result<()> {
    let mut archive = Archive::open("./resources/Zip Test 7-Zip BZip2 Ultra.zip").await?;

    let files = archive.list_files().await?;

    for file in files {
        debug!("{}", file.file_name);
        debug!("  compression: {:?}", file.compression);
        debug!("  min_version: {}", file.min_version);
        debug!("  gp_flag: {}", file.gp_flag);
        debug!("  comp_size: {}", file.compressed_size);
        debug!("  uncomp_size: {}", file.uncompressed_size);

        if file.compression != CompressionType::None {
            let contents = file.read(&mut archive).await?;

            debug!("{contents}");
        }
    }

    debug!("\n{:#?}", archive.info());

    Ok(())
}
