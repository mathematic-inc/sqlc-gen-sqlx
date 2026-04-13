#[path = "e2e/support/mod.rs"]
mod support;

#[tokio::test]
async fn generated_code_compiles_and_runs_against_postgres()
-> Result<(), Box<dyn std::error::Error>> {
    let postgres = support::start_postgres().await?;
    let database_url = support::database_url(&postgres).await?;

    for case in support::load_cases()? {
        let tempdir = tempfile::TempDir::new()?;
        let crate_root = support::write_generated_crate(&tempdir, &case)?;
        if let Err(err) = support::run_generated_crate(&crate_root, &case, &database_url) {
            return Err(format!("e2e case '{}' failed: {}", case.name, err).into());
        }
    }

    Ok(())
}
