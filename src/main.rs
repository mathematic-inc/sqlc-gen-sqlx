fn main() {
    if let Err(e) = sqlc_gen_sqlx::run() {
        eprintln!("sqlc-gen-sqlx: {e}");
        std::process::exit(1);
    }
}
