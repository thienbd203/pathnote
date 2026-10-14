#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    let db = sea_orm::Database::connect(std::env::var("DATABASE_URL")?).await?;
    let root = std::path::PathBuf::from(
        std::env::var("MEDIA_ROOT").unwrap_or_else(|_| ".storage/media".into()),
    );
    let report = server::clean_orphan_media(&db, &root, server::MEDIA_CLEANUP_GRACE).await?;
    println!(
        "Scanned {}, removed {} orphan files.",
        report.scanned, report.removed
    );
    db.close().await?;
    Ok(())
}
