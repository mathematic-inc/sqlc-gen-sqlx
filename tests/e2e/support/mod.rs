use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

use serde_json::json;
use tempfile::TempDir;
use testcontainers_modules::{
    postgres::Postgres,
    testcontainers::{ContainerAsync, runners::AsyncRunner},
};

#[derive(Debug, Clone)]
pub struct Case {
    pub name: String,
    pub dir: PathBuf,
    pub config_name: String,
    pub expect_rs: Option<String>,
    pub expected_stderr: Option<String>,
}

pub async fn start_postgres() -> Result<ContainerAsync<Postgres>, Box<dyn Error>> {
    Ok(Postgres::default()
        .with_db_name("sqlc_test")
        .with_user("sqlc")
        .with_password("sqlc")
        .start()
        .await?)
}

pub async fn database_url(postgres: &ContainerAsync<Postgres>) -> Result<String, Box<dyn Error>> {
    Ok(format!(
        "postgres://sqlc:sqlc@{}:{}/sqlc_test",
        postgres.get_host().await?,
        postgres.get_host_port_ipv4(5432).await?
    ))
}

pub fn load_cases() -> Result<Vec<Case>, Box<dyn Error>> {
    let root = cases_root();
    let mut cases = Vec::new();
    collect_cases(&root, &root, &mut cases)?;
    cases.sort_by(|left, right| left.name.cmp(&right.name));

    if cases.is_empty() {
        return Err(format!("no e2e cases found in {}", root.display()).into());
    }

    Ok(cases)
}

pub fn write_generated_crate(dir: &TempDir, case: &Case) -> Result<PathBuf, Box<dyn Error>> {
    let root = dir.path().to_path_buf();
    write_file(&root.join("Cargo.toml"), &crate_manifest(&case.name))?;
    write_file(&root.join("src/lib.rs"), "")?;
    copy_fixture_dir(&case.dir, &root)?;

    if let Some(expect_rs) = &case.expect_rs {
        write_file(&root.join("tests/runtime.rs"), expect_rs)?;
    }

    Ok(root)
}

pub fn run_generated_crate(
    crate_root: &Path,
    case: &Case,
    database_url: &str,
) -> Result<(), Box<dyn Error>> {
    let generate = command_output(
        Command::new("sqlc")
            .arg("generate")
            .arg("--file")
            .arg(&case.config_name)
            .current_dir(crate_root),
    )?;

    if let Some(expected_stderr) = &case.expected_stderr {
        if generate.status.success() {
            return Err("sqlc generate succeeded but failure was expected".into());
        }

        let actual = normalize_stderr(&String::from_utf8_lossy(&generate.stderr));
        let expected = normalize_stderr(expected_stderr);
        if actual != expected {
            return Err(format!(
                "stderr mismatch\nexpected:\n{}\n\nactual:\n{}",
                expected, actual
            )
            .into());
        }

        return Ok(());
    }

    if !generate.status.success() {
        return Err(format!(
            "sqlc generate failed\nstatus: {}\nstdout:\n{}\nstderr:\n{}",
            generate.status,
            String::from_utf8_lossy(&generate.stdout),
            String::from_utf8_lossy(&generate.stderr)
        )
        .into());
    }

    if case.expect_rs.is_none() {
        return Err("positive e2e cases must provide expect.rs".into());
    }

    run_command(
        Command::new("cargo")
            .arg("test")
            .arg("--color")
            .arg("never")
            .current_dir(crate_root)
            .env("DATABASE_URL", database_url),
        "generated crate tests",
    )
}

fn collect_cases(root: &Path, dir: &Path, cases: &mut Vec<Case>) -> Result<(), Box<dyn Error>> {
    let mut config_name = None::<String>;
    let mut subdirs = Vec::new();

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            subdirs.push(path);
            continue;
        }

        let name = entry.file_name().to_string_lossy().into_owned();
        if is_config_name(&name) && config_name.replace(name).is_some() {
            return Err(format!("multiple sqlc config files found in {}", dir.display()).into());
        }
    }

    if let Some(config_name) = config_name {
        cases.push(load_case(root, dir, &config_name)?);
        return Ok(());
    }

    for subdir in subdirs {
        collect_cases(root, &subdir, cases)?;
    }

    Ok(())
}

fn load_case(root: &Path, dir: &Path, config_name: &str) -> Result<Case, Box<dyn Error>> {
    let expect_path = dir.join("expect.rs");
    let stderr_path = dir.join("stderr.txt");
    let expect_rs = expect_path
        .is_file()
        .then(|| fs::read_to_string(&expect_path))
        .transpose()?;
    let expected_stderr = stderr_path
        .is_file()
        .then(|| fs::read_to_string(&stderr_path))
        .transpose()?;

    if expect_rs.is_none() && expected_stderr.is_none() {
        return Err(format!(
            "e2e fixture '{}' must define expect.rs or stderr.txt",
            dir.display()
        )
        .into());
    }

    Ok(Case {
        name: dir.strip_prefix(root)?.display().to_string(),
        dir: dir.to_path_buf(),
        config_name: config_name.to_string(),
        expect_rs,
        expected_stderr,
    })
}

fn is_config_name(name: &str) -> bool {
    matches!(name, "sqlc.yaml" | "sqlc.yml" | "sqlc.json")
}

fn copy_fixture_dir(source: &Path, dest: &Path) -> Result<(), Box<dyn Error>> {
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();

        if entry.file_type()?.is_dir() {
            copy_fixture_dir(&path, &dest.join(&name))?;
            continue;
        }

        if matches!(name.as_str(), "expect.rs" | "stderr.txt" | "exec.json") {
            continue;
        }

        let contents = fs::read_to_string(&path)?;
        let rewritten = if is_config_name(&name) {
            contents.replace(
                "{{plugin_binary}}",
                &json!(env!("CARGO_BIN_EXE_sqlc-gen-sqlx")).to_string(),
            )
        } else {
            contents
        };
        write_file(&dest.join(&name), &rewritten)?;
    }

    Ok(())
}

fn command_output(command: &mut Command) -> Result<Output, Box<dyn Error>> {
    Ok(command.output()?)
}

fn run_command(command: &mut Command, description: &str) -> Result<(), Box<dyn Error>> {
    let output = command_output(command)?;

    if !output.status.success() {
        return Err(format!(
            "{description} failed\nstatus: {}\nstdout:\n{}\nstderr:\n{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }

    Ok(())
}

fn normalize_stderr(input: &str) -> String {
    input
        .replace('\r', "")
        .replace('\\', "/")
        .trim()
        .to_string()
}

fn write_file(path: &Path, contents: &str) -> Result<(), Box<dyn Error>> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, contents)?;
    Ok(())
}

fn cases_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/e2e/cases")
}

fn crate_manifest(case_name: &str) -> String {
    include_str!("../fixtures/Cargo.toml.tmpl")
        .replace("{{package_name}}", &crate_package_name(case_name))
}

fn crate_package_name(case_name: &str) -> String {
    case_name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') {
                ch
            } else {
                '-'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::load_cases;

    #[test]
    fn discovers_cases_from_directory() -> Result<(), Box<dyn std::error::Error>> {
        let cases = load_cases()?;
        assert!(cases.iter().any(|case| case.name == "one_named_params"));
        assert!(cases.iter().any(|case| case.name == "copyfrom"));
        assert!(
            cases
                .iter()
                .any(|case| case.name == "config_output_name/postgresql")
        );
        Ok(())
    }
}
