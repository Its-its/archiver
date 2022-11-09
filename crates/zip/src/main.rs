use zip_archiver::{Archive, Result, CompressionType};

#[tokio::main]
async fn main() -> Result<()> {
    let mut archive = Archive::open("./resources/Zip Test 7-Zip BZip2 Ultra.zip").await?;

    let files = archive.list_files().await?;

    for file in files {
        println!("{}", file.file_name);
        println!("  compression: {:?}", file.compression);
        println!("  min_version: {}", file.min_version);
        println!("  gp_flag: {}", file.gp_flag);
        println!("  comp_size: {}", file.compressed_size);
        println!("  uncomp_size: {}", file.uncompressed_size);

        if file.compression != CompressionType::None {
            let contents = file.read(&mut archive).await?;

            println!("{contents}");
        }
    }

    println!("\n{:#?}", archive.info());

    Ok(())
}
