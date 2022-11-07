use zip_archiver::{Archive, Result};

#[tokio::main]
async fn main() -> Result<()> {
    let mut archive = Archive::open("./resources/Zip Test 7-Zip.zip").await?;

    println!("{:#?}", archive.info());

    let files = archive.list_files().await?;

    println!("{:#?}", files);

    Ok(())
}
