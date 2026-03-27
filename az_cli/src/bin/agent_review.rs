#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    az_cli::agent_review::run().await
}
