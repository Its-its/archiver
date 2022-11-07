use zip_archiver::Archive;

#[tokio::main]
async fn main() {
    let mut archive = Archive::open().await;

    println!("{:#?}", archive.info());

    archive.read_file().await;
}
