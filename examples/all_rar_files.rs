use rar_archiver::{Archive, Result};
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

    let _archive = Archive::open("./resources/rar/RAR4 Test Winrar Best 4096KB.rar").await?;

    // debug!("{:#?}", &archive.files[0]);

    Ok(())
}