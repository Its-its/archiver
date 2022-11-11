use tracing::debug;
use zip_archiver::{Archive, Result};
use tracing::{subscriber::set_global_default, Level};
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() -> Result<()> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::TRACE)
        .with_file(false)
        .with_line_number(true)
        .finish();

    #[allow(clippy::expect_used)]
    set_global_default(subscriber).expect("setting default subscriber failed");

    let mut archive = Archive::open("./resources/zip/Zip Test 7-Zip BZip2 Ultra.zip").await?;

    let files = archive.list_files().await?;

    for file in files {
        debug!("{}", file.file_name);
        debug!("   GP FLAG: {:#X}", file.gp_flag);
        debug!("   compression: {:?}", file.compression);
        debug!("   min_version: {}", file.min_version);
        debug!("   gp_flag: {}", file.gp_flag);
        debug!("   comp_size: {}", file.compressed_size);
        debug!("   uncomp_size: {}", file.uncompressed_size);

        // if file.min_version.is_file() {
        //     let contents = file.read(&mut archive).await?;

        //     debug!("{contents}");
        // }
    }

    debug!("{:#?}", archive.info());

    Ok(())
}
