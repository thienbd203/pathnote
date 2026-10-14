#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    let db = sea_orm::Database::connect(std::env::var("DATABASE_URL")?).await?;
    let count = server::rebuild_vocabulary(&db).await?;
    println!("Synchronized {count} lesson notes.");
    db.close().await?;
    Ok(())
}
