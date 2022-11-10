use rar_archiver::{Archive, Result};

#[tokio::main]
async fn main() -> Result<()> {
    let mut archive = Archive::open("./resources/rar/RAR Test Winrar Best 128KB.rar").await?;

    // let files = archive.list_files().await?;

    // for file in files {
    //     trace!("{}", file.file_name);
    //     trace!("  compression: {:?}", file.compression);
    //     trace!("  min_version: {}", file.min_version);
    //     trace!("  gp_flag: {}", file.gp_flag);
    //     trace!("  comp_size: {}", file.compressed_size);
    //     trace!("  uncomp_size: {}", file.uncompressed_size);

    //     if file.compression != CompressionType::None {
    //         let contents = file.read(&mut archive).await?;

    //         // trace!("{contents}");
    //     }
    // }

    // trace!("\n{:#?}", archive.info());

    Ok(())
}
