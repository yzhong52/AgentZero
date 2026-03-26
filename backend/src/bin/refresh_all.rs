#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    agent_zero_backend::cli::refresh_all::run().await
}
