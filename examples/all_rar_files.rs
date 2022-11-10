use rar_archiver::{Archive, Result};
use tracing::{subscriber::set_global_default, Level, debug};
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

    let mut archive = Archive::open("./resources/rar/RAR Test Winrar Best 128KB.rar").await?;

    // debug!("{:#?}", &archive.files[0]);

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



fn extract_vint(buffer: &[u8]) -> (usize, u64) {
    let mut shift_amount: u64 = 0;
    let mut decoded_value: u64 = 0;

    let len = buffer.iter().take_while(|v| is_cont_bit(**v)).count() + 1;

    for &next_byte in &buffer[0..len] {
        decoded_value |= ((next_byte & 0b0111_1111) as u64) << shift_amount;

        // See if we're supposed to keep reading
        if (next_byte & 0b1000_0000) != 0 {
            shift_amount += 7;
        } else {
            break;
        }
    }

    (len, decoded_value)
}

fn is_cont_bit(value: u8) -> bool {
    // 0b1000_0000 -> 0b0000_0001
    value >> 7 == 1
}