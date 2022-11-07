use zip_archiver::Archive;

#[tokio::main]
async fn main() {
    let mut archive = Archive::open("./resources/Zip Test 7-Zip.zip").await;

    println!("{:#?}", archive.info());

    let files = archive.list_files().await;

    println!("{:#?}", files);
}
